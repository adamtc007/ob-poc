/**
 * CoverageMap — entity list with governance coverage badges.
 *
 * Badge colors: governed (green), unowned (gray), stale (amber).
 */

interface CoverageEntry {
  entity_name: string;
  entity_type?: string;
  fqn?: string;
  status: "governed" | "unowned" | "stale" | string;
  owner?: string;
  last_reviewed?: string;
}

interface Props {
  data: unknown;
}

const STATUS_STYLES: Record<string, { bg: string; text: string; label: string }> = {
  governed: {
    bg: "bg-emerald-500/20 border-emerald-500/40",
    text: "text-emerald-400",
    label: "Governed",
  },
  unowned: {
    bg: "bg-gray-500/20 border-gray-500/40",
    text: "text-gray-400",
    label: "Unowned",
  },
  stale: {
    bg: "bg-amber-500/20 border-amber-500/40",
    text: "text-amber-400",
    label: "Stale",
  },
};

const DEFAULT_STYLE = {
  bg: "bg-[var(--bg-active)] border-[var(--border-secondary)]",
  text: "text-[var(--text-secondary)]",
  label: "Unknown",
};

export function CoverageMap({ data }: Props) {
  if (!data || typeof data !== "object") {
    return (
      <div className="text-xs text-[var(--text-secondary)]">
        No coverage data
      </div>
    );
  }

  const root = data as Record<string, unknown>;
  const entries = (root.entries as CoverageEntry[]) ?? [];

  if (entries.length === 0) {
    return (
      <div className="text-xs text-[var(--text-secondary)]">
        No entities to display
      </div>
    );
  }

  // Summary counts
  const counts = entries.reduce<Record<string, number>>((acc, e) => {
    acc[e.status] = (acc[e.status] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <div className="space-y-2">
      {/* Summary bar */}
      <div className="flex gap-3 text-[10px]">
        {Object.entries(counts).map(([status, count]) => {
          const style = STATUS_STYLES[status] ?? DEFAULT_STYLE;
          return (
            <span key={status} className={style.text}>
              {style.label}: {count}
            </span>
          );
        })}
      </div>

      {/* Entity list */}
      <div className="space-y-1">
        {entries.map((entry, i) => {
          const style = STATUS_STYLES[entry.status] ?? DEFAULT_STYLE;
          return (
            <div
              key={entry.fqn ?? i}
              className="flex items-center gap-2 text-xs"
            >
              <span
                className={`shrink-0 px-1.5 py-0.5 rounded border text-[9px] font-semibold ${style.bg} ${style.text}`}
              >
                {style.label}
              </span>
              <div className="flex-1 min-w-0">
                <span className="text-[var(--text-primary)] truncate block">
                  {entry.entity_name}
                </span>
                {entry.entity_type && (
                  <span className="text-[10px] text-[var(--text-muted)]">
                    {entry.entity_type}
                  </span>
                )}
              </div>
              {entry.owner && (
                <span className="shrink-0 text-[10px] text-[var(--text-muted)]">
                  {entry.owner}
                </span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default CoverageMap;
