/**
 * LocationHeader — typed orientation display.
 *
 * Shows session mode, view level, focus identity, lens state, and action count.
 */

import type { OrientationContract } from "../../../types/observatory";

interface Props {
  orientation: OrientationContract | null;
}

const MODE_COLORS: Record<string, string> = {
  research: "text-blue-400",
  governed: "text-emerald-400",
  maintenance: "text-amber-400",
};

export function LocationHeader({ orientation }: Props) {
  if (!orientation) {
    return (
      <div className="flex items-center gap-3 px-4 py-2 border-b border-[var(--border-primary)] text-[var(--text-secondary)] text-sm">
        Loading orientation...
      </div>
    );
  }

  const modeClass =
    MODE_COLORS[orientation.session_mode] ?? "text-[var(--text-secondary)]";
  const enabledActions = orientation.available_actions.filter(
    (a) => a.enabled,
  ).length;

  return (
    <div className="flex items-center gap-3 px-4 py-2 border-b border-[var(--border-primary)] text-sm">
      <span className={`font-semibold uppercase text-xs ${modeClass}`}>
        {orientation.session_mode}
      </span>
      <span className="text-[var(--text-muted)]">&middot;</span>
      <span className="text-[var(--text-primary)] font-medium capitalize">
        {orientation.view_level}
      </span>
      <span className="text-[var(--text-muted)]">&middot;</span>
      <span className="text-[var(--text-primary)]">
        {orientation.focus_identity.business_label}
      </span>
      {orientation.lens.depth_probe && (
        <>
          <span className="text-[var(--text-muted)]">&middot;</span>
          <span className="text-[var(--text-secondary)] text-xs">
            Lens: {orientation.lens.depth_probe}
          </span>
        </>
      )}
      <span className="ml-auto text-[var(--text-secondary)] text-xs">
        {enabledActions} actions
      </span>
    </div>
  );
}

export default LocationHeader;
