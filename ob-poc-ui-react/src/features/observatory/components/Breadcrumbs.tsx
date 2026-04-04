/**
 * Breadcrumbs — navigation history trail from OrientationContract entries.
 */

import type { OrientationContract } from "../../../types/observatory";

interface Props {
  history: OrientationContract[];
}

export function Breadcrumbs({ history }: Props) {
  if (history.length === 0) return null;

  return (
    <div className="flex items-center gap-1 px-4 py-1 text-xs border-b border-[var(--border-primary)] overflow-x-auto">
      {history.map((entry, i) => (
        <span key={i} className="flex items-center gap-1 shrink-0">
          {i > 0 && (
            <span className="text-[var(--text-muted)]">&rsaquo;</span>
          )}
          <button
            className={`px-1.5 py-0.5 rounded ${
              i === history.length - 1
                ? "text-[var(--text-primary)] font-medium"
                : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
            }`}
          >
            {entry.focus_identity.business_label || entry.view_level}
          </button>
        </span>
      ))}
    </div>
  );
}

export default Breadcrumbs;
