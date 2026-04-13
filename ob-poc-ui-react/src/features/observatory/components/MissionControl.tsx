/**
 * MissionControl — health metrics dashboard for the Observatory.
 */

import { useQuery } from "@tanstack/react-query";
import { observatoryApi } from "../../../api/observatory";
import { queryKeys } from "../../../lib/query";

interface Props {
  sessionId: string;
}

export function MissionControl({ sessionId: _sessionId }: Props) {
  const { data: health } = useQuery({
    queryKey: queryKeys.observatory.health(),
    queryFn: () => observatoryApi.getHealth(),
    refetchInterval: 10000,
  });

  if (!health) {
    return (
      <div className="p-6 text-[var(--text-secondary)]">
        Loading health metrics...
      </div>
    );
  }

  const metrics = [
    {
      label: "Pending Changesets",
      value: health.pending_changesets,
      warn: health.pending_changesets > 5,
    },
    {
      label: "Stale Dry Runs",
      value: health.stale_dryruns,
      warn: health.stale_dryruns > 0,
    },
    {
      label: "Active Snapshots",
      value: health.active_snapshots,
      warn: false,
    },
    {
      label: "Archived",
      value: health.archived_changesets,
      warn: false,
    },
    {
      label: "Embedding Freshness",
      value:
        health.embedding_freshness_hours != null
          ? `${health.embedding_freshness_hours.toFixed(1)}h`
          : "\u2014",
      warn: (health.embedding_freshness_hours ?? 0) > 24,
    },
    {
      label: "Outbox Depth",
      value: health.outbox_depth ?? "\u2014",
      warn: (health.outbox_depth ?? 0) > 100,
    },
  ];

  return (
    <div className="p-6">
      <h2 className="text-lg font-semibold text-[var(--text-primary)] mb-4">
        Mission Control
      </h2>
      <div className="grid grid-cols-3 gap-4">
        {metrics.map((m) => (
          <div
            key={m.label}
            className={`rounded-lg border p-4 ${
              m.warn
                ? "border-amber-500/40 bg-amber-500/5"
                : "border-[var(--border-secondary)] bg-[var(--bg-secondary)]"
            }`}
          >
            <div className="text-2xl font-bold text-[var(--text-primary)]">
              {m.value}
            </div>
            <div className="text-xs text-[var(--text-secondary)] mt-1">
              {m.label}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default MissionControl;
