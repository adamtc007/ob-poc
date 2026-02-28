/**
 * SemOsContextPanel - Right-side panel showing registry stats and recent changesets
 */

import { useQuery } from "@tanstack/react-query";
import { Loader2, Database, RefreshCw } from "lucide-react";
import { semOsApi } from "../../../api/semOs";
import { queryKeys } from "../../../lib/query";
import { cn, formatDate } from "../../../lib/utils";

interface SemOsContextPanelProps {
  className?: string;
}

const statusColors: Record<string, string> = {
  draft: "text-[var(--text-muted)]",
  validated: "text-[var(--accent-blue)]",
  dry_run_passed: "text-[var(--accent-blue)]",
  published: "text-[var(--accent-green)]",
  rejected: "text-[var(--accent-red)]",
  under_review: "text-[var(--accent-yellow)]",
  approved: "text-[var(--accent-green)]",
};

export function SemOsContextPanel({ className }: SemOsContextPanelProps) {
  const {
    data: context,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: queryKeys.semOs.context(),
    queryFn: semOsApi.getContext,
    refetchInterval: 10_000,
  });

  return (
    <div className={cn("flex flex-col overflow-hidden", className)}>
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[var(--border-primary)] px-4 py-3">
        <h2 className="font-semibold text-[var(--text-primary)] text-sm">
          Registry
        </h2>
        <button
          onClick={() => refetch()}
          className="rounded p-1 text-[var(--text-muted)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
          title="Refresh"
        >
          <RefreshCw size={14} />
        </button>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center py-8">
          <Loader2 className="h-5 w-5 animate-spin text-[var(--text-muted)]" />
        </div>
      ) : !context ? (
        <div className="px-4 py-4 text-xs text-[var(--text-muted)]">
          No context available
        </div>
      ) : (
        <div className="flex-1 overflow-auto">
          {/* Agent Mode Badge */}
          <div className="border-b border-[var(--border-primary)] px-4 py-2">
            <div className="flex items-center gap-2">
              <span className="text-xs text-[var(--text-muted)]">Mode:</span>
              <span
                className={cn(
                  "rounded px-1.5 py-0.5 text-xs font-medium",
                  context.agent_mode === "Governed"
                    ? "bg-[var(--accent-green)]/10 text-[var(--accent-green)]"
                    : "bg-[var(--accent-blue)]/10 text-[var(--accent-blue)]",
                )}
              >
                {context.agent_mode}
              </span>
            </div>
          </div>

          {/* Registry Stats */}
          <div className="border-b border-[var(--border-primary)] px-4 py-3">
            <h3 className="mb-2 flex items-center gap-1.5 text-xs font-medium text-[var(--text-secondary)]">
              <Database size={12} />
              Object Counts
            </h3>
            <div className="space-y-1">
              {Object.entries(context.registry_stats).map(([type, count]) => (
                <div
                  key={type}
                  className="flex items-center justify-between text-xs"
                >
                  <span className="text-[var(--text-secondary)]">{type}</span>
                  <span className="font-mono text-[var(--text-primary)]">
                    {count}
                  </span>
                </div>
              ))}
              {Object.keys(context.registry_stats).length === 0 && (
                <p className="text-xs text-[var(--text-muted)]">
                  No registry data
                </p>
              )}
            </div>
          </div>

          {/* Recent Changesets */}
          <div className="px-4 py-3">
            <h3 className="mb-2 text-xs font-medium text-[var(--text-secondary)]">
              Recent Changesets
            </h3>
            <div className="space-y-2">
              {context.recent_changesets.map((cs) => (
                <div
                  key={cs.id}
                  className="rounded border border-[var(--border-primary)] p-2"
                >
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-medium text-[var(--text-primary)] truncate">
                      {cs.title || cs.id.slice(0, 8)}
                    </span>
                    <span
                      className={cn(
                        "text-[10px] font-medium",
                        statusColors[cs.status] || "text-[var(--text-muted)]",
                      )}
                    >
                      {cs.status}
                    </span>
                  </div>
                  <div className="mt-1 flex items-center gap-2 text-[10px] text-[var(--text-muted)]">
                    <span>{cs.entry_count} items</span>
                    <span>{formatDate(cs.created_at)}</span>
                  </div>
                </div>
              ))}
              {context.recent_changesets.length === 0 && (
                <p className="text-xs text-[var(--text-muted)]">
                  No recent changesets
                </p>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default SemOsContextPanel;
