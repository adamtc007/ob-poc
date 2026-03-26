/**
 * RunbookPlanReview — Displays a compiled runbook plan for review and approval.
 *
 * Shows plan steps in a vertical timeline, forward-reference bindings,
 * approve/cancel actions, step-by-step execution with narration, and
 * aggregate completion summary.
 */

import { useState, useCallback, useEffect } from "react";
import {
  CheckCircle,
  XCircle,
  Clock,
  Play,
  Loader,
  ArrowRight,
  Link2,
  Ban,
  SkipForward,
} from "lucide-react";
import { cn } from "../../lib/utils";
import { runbookPlanApi } from "../../api/runbookPlan";
import type {
  RunbookPlan,
  RunbookPlanStep,
  PlanStepStatus,
  EntityBinding,
  RunbookPlanStatus,
} from "../../api/runbookPlan";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface RunbookPlanReviewProps {
  sessionId: string;
  /** If provided, use this plan directly instead of fetching. */
  initialPlan?: RunbookPlan;
  /** Called when the plan is approved. */
  onApproved?: () => void;
  /** Called when the plan is cancelled. */
  onCancelled?: () => void;
  /** Called when all steps have been executed. */
  onCompleted?: () => void;
}

// ---------------------------------------------------------------------------
// Status helpers
// ---------------------------------------------------------------------------

const statusIcon: Record<PlanStepStatus, React.ReactNode> = {
  pending: <Clock size={16} className="text-[var(--text-muted)]" />,
  ready: <Play size={16} className="text-[var(--accent-blue)]" />,
  executing: <Loader size={16} className="animate-spin text-[var(--accent-yellow)]" />,
  succeeded: <CheckCircle size={16} className="text-[var(--accent-green)]" />,
  failed: <XCircle size={16} className="text-[var(--accent-red)]" />,
  skipped: <SkipForward size={16} className="text-[var(--text-muted)]" />,
};

const statusLabel: Record<PlanStepStatus, string> = {
  pending: "Pending",
  ready: "Ready",
  executing: "Executing",
  succeeded: "Succeeded",
  failed: "Failed",
  skipped: "Skipped",
};

const workspaceBadgeColors: Record<string, string> = {
  cbu: "bg-blue-100 text-blue-800 border-blue-300",
  deal: "bg-purple-100 text-purple-800 border-purple-300",
  kyc: "bg-amber-100 text-amber-800 border-amber-300",
  on_boarding: "bg-green-100 text-green-800 border-green-300",
  instrument_matrix: "bg-cyan-100 text-cyan-800 border-cyan-300",
  product_maintenance: "bg-rose-100 text-rose-800 border-rose-300",
};

function workspaceBadge(workspace: string) {
  const colors = workspaceBadgeColors[workspace] ?? "bg-gray-100 text-gray-700 border-gray-300";
  return colors;
}

function isForwardRef(binding: EntityBinding): binding is { kind: "forward_ref"; source_step: number; output_field: string } {
  return binding.kind === "forward_ref";
}

/** Extract the plan-level status string. */
function planStatusKey(status: RunbookPlanStatus): string {
  if (typeof status === "string") return status;
  return status.status;
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function BindingBadge({ binding }: { binding: EntityBinding }) {
  if (!isForwardRef(binding)) return null;
  return (
    <span className="inline-flex items-center gap-1 rounded bg-indigo-50 px-1.5 py-0.5 text-xs font-mono text-indigo-700 border border-indigo-200">
      <Link2 size={10} />
      step {binding.source_step}.{binding.output_field}
    </span>
  );
}

function DependencyArrows({ dependsOn }: { dependsOn: number[] }) {
  if (dependsOn.length === 0) return null;
  return (
    <div className="flex items-center gap-1 text-xs text-[var(--text-muted)]">
      <ArrowRight size={12} />
      <span>depends on: {dependsOn.map((d) => `#${d}`).join(", ")}</span>
    </div>
  );
}

function StepRow({
  step,
  isActive,
}: {
  step: RunbookPlanStep;
  isActive: boolean;
}) {
  return (
    <div
      className={cn(
        "relative flex gap-3 rounded-lg border p-3 transition-colors",
        isActive
          ? "border-[var(--accent-blue)] bg-[var(--accent-blue)]/5"
          : "border-[var(--border-primary)] bg-[var(--bg-secondary)]"
      )}
    >
      {/* Sequence number + status icon */}
      <div className="flex flex-col items-center gap-1">
        <span className="flex h-7 w-7 items-center justify-center rounded-full bg-[var(--bg-tertiary)] text-xs font-bold text-[var(--text-secondary)]">
          {step.seq}
        </span>
        {statusIcon[step.status]}
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0 space-y-1">
        {/* Header: workspace badge + verb FQN */}
        <div className="flex flex-wrap items-center gap-2">
          <span
            className={cn(
              "inline-block rounded border px-1.5 py-0.5 text-xs font-medium",
              workspaceBadge(step.workspace)
            )}
          >
            {step.workspace}
          </span>
          <span className="font-mono text-sm font-medium text-[var(--text-primary)]">
            {step.verb.verb_fqn}
          </span>
          <span className="text-xs text-[var(--text-muted)]">
            {statusLabel[step.status]}
          </span>
        </div>

        {/* Sentence */}
        <p className="text-sm text-[var(--text-primary)]">{step.sentence}</p>

        {/* Expected effect */}
        {step.expected_effect && (
          <p className="text-xs text-[var(--text-secondary)] italic">
            Effect: {step.expected_effect}
          </p>
        )}

        {/* Forward ref bindings */}
        {isForwardRef(step.subject_binding) && (
          <div className="flex items-center gap-1">
            <BindingBadge binding={step.subject_binding} />
          </div>
        )}

        {/* Dependencies */}
        <DependencyArrows dependsOn={step.depends_on} />
      </div>
    </div>
  );
}

/** Narration result displayed after a step executes. */
function StepNarrationCard({ result }: { result: Record<string, unknown> }) {
  const narration = result.narration as
    | { outcome: string; what_changed?: string; state_now?: string; what_next?: string[]; error?: string; recovery_hint?: string; reason?: string }
    | undefined;

  if (!narration) {
    // Fallback: show raw step result
    const success = result.step_status === "succeeded";
    return (
      <div
        className={cn(
          "rounded border px-3 py-2 text-sm",
          success
            ? "border-green-200 bg-green-50 text-green-800"
            : "border-red-200 bg-red-50 text-red-800"
        )}
      >
        Step {String(result.step_seq ?? "?")} — {success ? "Succeeded" : "Failed"}
        {result.error ? (
          <span className="block text-xs mt-1">{String(result.error)}</span>
        ) : null}
      </div>
    );
  }

  if (narration.outcome === "success") {
    return (
      <div className="rounded border border-green-200 bg-green-50 px-3 py-2 text-sm text-green-900 space-y-1">
        <div className="font-medium">{narration.what_changed}</div>
        <div className="text-xs text-green-700">{narration.state_now}</div>
        {narration.what_next && narration.what_next.length > 0 && (
          <ul className="text-xs text-green-600 list-disc list-inside">
            {narration.what_next.map((n, i) => (
              <li key={i}>{n}</li>
            ))}
          </ul>
        )}
      </div>
    );
  }

  if (narration.outcome === "failed") {
    return (
      <div className="rounded border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-900 space-y-1">
        <div className="font-medium">Failed: {narration.error}</div>
        {narration.recovery_hint && (
          <div className="text-xs text-red-700">{narration.recovery_hint}</div>
        )}
      </div>
    );
  }

  return (
    <div className="rounded border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-700">
      Skipped: {narration.reason ?? "No reason given"}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Aggregate Summary
// ---------------------------------------------------------------------------

function AggregateSummary({
  plan,
  stepResults,
}: {
  plan: RunbookPlan;
  stepResults: Record<string, unknown>[];
}) {
  const succeeded = plan.steps.filter((s) => s.status === "succeeded").length;
  const failed = plan.steps.filter((s) => s.status === "failed").length;
  const skipped = plan.steps.filter((s) => s.status === "skipped").length;
  const total = plan.steps.length;

  return (
    <div className="rounded-lg border border-indigo-200 bg-indigo-50 p-4 space-y-2">
      <div className="text-sm font-semibold text-indigo-900">
        Plan Complete
      </div>
      <div className="flex gap-4 text-xs text-indigo-700">
        <span>{total} total steps</span>
        <span className="text-green-700">{succeeded} succeeded</span>
        {failed > 0 && <span className="text-red-700">{failed} failed</span>}
        {skipped > 0 && <span className="text-gray-600">{skipped} skipped</span>}
      </div>
      {stepResults.length > 0 && (
        <div className="space-y-1 pt-1">
          {stepResults.map((r, i) => (
            <StepNarrationCard key={i} result={r} />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function RunbookPlanReview({
  sessionId,
  initialPlan,
  onApproved,
  onCancelled,
  onCompleted,
}: RunbookPlanReviewProps) {
  const [plan, setPlan] = useState<RunbookPlan | null>(initialPlan ?? null);
  const [loading, setLoading] = useState(!initialPlan);
  const [error, setError] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState(false);
  const [stepResults, setStepResults] = useState<Record<string, unknown>[]>([]);

  // Fetch plan on mount if not provided
  useEffect(() => {
    if (initialPlan) return;
    let cancelled = false;
    setLoading(true);
    runbookPlanApi
      .getRunbookPlan(sessionId)
      .then((p) => {
        if (!cancelled) setPlan(p);
      })
      .catch((e) => {
        if (!cancelled) setError(e?.message ?? "Failed to load plan");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId, initialPlan]);

  // Refresh plan from server (after mutations)
  const refreshPlan = useCallback(async () => {
    try {
      const updated = await runbookPlanApi.getRunbookPlan(sessionId);
      setPlan(updated);
    } catch {
      // Ignore refresh errors — plan may have been cancelled
    }
  }, [sessionId]);

  // --- Actions ---

  const handleApprove = useCallback(async () => {
    setActionLoading(true);
    setError(null);
    try {
      await runbookPlanApi.approveRunbookPlan(sessionId);
      await refreshPlan();
      onApproved?.();
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : "Approve failed";
      setError(msg);
    } finally {
      setActionLoading(false);
    }
  }, [sessionId, refreshPlan, onApproved]);

  const handleCancel = useCallback(async () => {
    setActionLoading(true);
    setError(null);
    try {
      await runbookPlanApi.cancelRunbookPlan(sessionId);
      await refreshPlan();
      onCancelled?.();
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : "Cancel failed";
      setError(msg);
    } finally {
      setActionLoading(false);
    }
  }, [sessionId, refreshPlan, onCancelled]);

  const handleExecuteNext = useCallback(async () => {
    setActionLoading(true);
    setError(null);
    try {
      const result = await runbookPlanApi.executeRunbookPlanStep(sessionId);
      setStepResults((prev) => [...prev, result]);
      await refreshPlan();

      // Check if plan is now completed
      const status = await runbookPlanApi.getRunbookStatus(sessionId);
      const key = typeof status.status === "string" ? status.status : (status.status as RunbookPlanStatus).status;
      if (key === "completed") {
        onCompleted?.();
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : "Execution failed";
      setError(msg);
      await refreshPlan();
    } finally {
      setActionLoading(false);
    }
  }, [sessionId, refreshPlan, onCompleted]);

  // --- Render ---

  if (loading) {
    return (
      <div className="flex items-center justify-center gap-2 py-8 text-sm text-[var(--text-muted)]">
        <Loader size={16} className="animate-spin" />
        Loading runbook plan...
      </div>
    );
  }

  if (!plan) {
    return (
      <div className="rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)] p-4 text-sm text-[var(--text-muted)]">
        {error ?? "No runbook plan available."}
      </div>
    );
  }

  const statusKey = planStatusKey(plan.status);
  const isApproved = statusKey === "approved" || statusKey === "executing";
  const isExecuting = statusKey === "executing";
  const isCompleted = statusKey === "completed";
  const isCancelled = statusKey === "cancelled";
  const isFailed = statusKey === "failed";
  const canApprove = statusKey === "compiled" || statusKey === "awaiting_approval";
  const canExecute = isApproved || isExecuting;
  const canCancel = canApprove || canExecute;

  // Determine which step is "active" (next to execute)
  const cursor =
    isExecuting && "cursor" in plan.status
      ? (plan.status as { cursor: number }).cursor
      : plan.steps.findIndex((s) => s.status === "pending" || s.status === "ready");

  // Collect forward-reference bindings for summary display
  const forwardRefs = plan.steps.filter((s) => isForwardRef(s.subject_binding));

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <div className="text-sm font-semibold text-[var(--text-primary)]">
            Runbook Plan
          </div>
          <div className="text-xs text-[var(--text-muted)]">
            {plan.steps.length} steps &middot; Plan {plan.id.slice(0, 12)}...
          </div>
        </div>
        <div
          className={cn(
            "rounded-full px-2.5 py-0.5 text-xs font-medium border",
            canApprove && "bg-amber-50 text-amber-800 border-amber-200",
            isApproved && "bg-blue-50 text-blue-800 border-blue-200",
            isCompleted && "bg-green-50 text-green-800 border-green-200",
            isCancelled && "bg-gray-50 text-gray-600 border-gray-200",
            isFailed && "bg-red-50 text-red-800 border-red-200"
          )}
        >
          {statusKey.replace(/_/g, " ")}
        </div>
      </div>

      {/* Forward-reference bindings summary */}
      {forwardRefs.length > 0 && (
        <div className="rounded border border-indigo-100 bg-indigo-50/50 px-3 py-2">
          <div className="text-xs font-medium text-indigo-800 mb-1">
            Forward Reference Bindings
          </div>
          <div className="space-y-0.5">
            {forwardRefs.map((s) => {
              const ref = s.subject_binding as {
                kind: "forward_ref";
                source_step: number;
                output_field: string;
              };
              return (
                <div key={s.seq} className="flex items-center gap-2 text-xs text-indigo-700">
                  <span>Step #{s.seq}</span>
                  <ArrowRight size={10} />
                  <span className="font-mono">
                    step {ref.source_step}.{ref.output_field}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Step timeline */}
      <div className="relative space-y-2">
        {/* Vertical connector line */}
        <div className="absolute left-[22px] top-4 bottom-4 w-px bg-[var(--border-primary)]" />

        {plan.steps.map((step) => (
          <StepRow
            key={step.seq}
            step={step}
            isActive={cursor === step.seq}
          />
        ))}
      </div>

      {/* Step narration results */}
      {stepResults.length > 0 && !isCompleted && (
        <div className="space-y-2">
          <div className="text-xs font-medium text-[var(--text-secondary)]">
            Execution Results
          </div>
          {stepResults.map((r, i) => (
            <StepNarrationCard key={i} result={r} />
          ))}
        </div>
      )}

      {/* Aggregate summary when complete */}
      {isCompleted && (
        <AggregateSummary plan={plan} stepResults={stepResults} />
      )}

      {/* Error */}
      {error && (
        <div className="rounded border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-800">
          {error}
        </div>
      )}

      {/* Action buttons */}
      {!isCompleted && !isCancelled && (
        <div className="flex gap-2 pt-1">
          {canApprove && (
            <button
              onClick={handleApprove}
              disabled={actionLoading}
              className="flex-1 rounded bg-[var(--accent-green)] px-4 py-2 text-sm font-medium text-white hover:bg-[var(--accent-green)]/80 disabled:opacity-50"
            >
              {actionLoading ? "Approving..." : "Approve"}
            </button>
          )}

          {canExecute && (
            <button
              onClick={handleExecuteNext}
              disabled={actionLoading}
              className="flex-1 flex items-center justify-center gap-1.5 rounded bg-[var(--accent-blue)] px-4 py-2 text-sm font-medium text-white hover:bg-[var(--accent-blue)]/80 disabled:opacity-50"
            >
              {actionLoading ? (
                <Loader size={14} className="animate-spin" />
              ) : (
                <Play size={14} />
              )}
              Execute Next Step
            </button>
          )}

          {canCancel && (
            <button
              onClick={handleCancel}
              disabled={actionLoading}
              className="flex items-center gap-1.5 rounded border border-[var(--border-secondary)] px-4 py-2 text-sm text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] disabled:opacity-50"
            >
              <Ban size={14} />
              Cancel
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default RunbookPlanReview;
