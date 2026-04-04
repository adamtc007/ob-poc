/**
 * ImpactGraph — list of affected data assets with impact type and severity.
 */

interface ImpactEntry {
  entity_name: string;
  entity_type?: string;
  impact_type: string;
  severity: string;
}

interface Props {
  data: unknown;
}

const SEVERITY_STYLES: Record<string, string> = {
  critical: "bg-red-500/20 text-red-400 border-red-500/40",
  high: "bg-orange-500/20 text-orange-400 border-orange-500/40",
  medium: "bg-amber-500/20 text-amber-400 border-amber-500/40",
  low: "bg-blue-500/20 text-blue-400 border-blue-500/40",
  info: "bg-[var(--bg-active)] text-[var(--text-secondary)] border-[var(--border-secondary)]",
};

export function ImpactGraph({ data }: Props) {
  if (!data || typeof data !== "object") {
    return (
      <div className="text-xs text-[var(--text-secondary)]">
        No impact data
      </div>
    );
  }

  const root = data as Record<string, unknown>;
  const entries = (root.entries as ImpactEntry[]) ?? [];

  if (entries.length === 0) {
    return (
      <div className="text-xs text-[var(--text-secondary)]">
        No affected assets
      </div>
    );
  }

  return (
    <div className="space-y-1.5">
      {entries.map((entry, i) => {
        const style =
          SEVERITY_STYLES[entry.severity] ?? SEVERITY_STYLES["info"];
        return (
          <div
            key={i}
            className={`flex items-center gap-2 px-2 py-1.5 rounded border text-xs ${style}`}
          >
            <div className="flex-1 min-w-0">
              <div className="font-medium text-[var(--text-primary)] truncate">
                {entry.entity_name}
              </div>
              {entry.entity_type && (
                <div className="text-[10px] text-[var(--text-muted)]">
                  {entry.entity_type}
                </div>
              )}
            </div>
            <span className="shrink-0 text-[10px] font-mono">
              {entry.impact_type}
            </span>
            <span
              className={`shrink-0 px-1.5 py-0.5 rounded text-[9px] font-semibold uppercase border ${style}`}
            >
              {entry.severity}
            </span>
          </div>
        );
      })}
    </div>
  );
}

export default ImpactGraph;
