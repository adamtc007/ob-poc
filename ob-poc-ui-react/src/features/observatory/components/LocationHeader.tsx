/**
 * LocationHeader — typed orientation display.
 *
 * Shows session mode, view level, focus identity, lens state, and action count.
 */

import { useState } from "react";
import type { OrientationContract } from "../../../types/observatory";
import { chatApi } from "../../../api/chat";
import { queryClient, queryKeys } from "../../../lib/query";

interface Props {
  orientation: OrientationContract | null;
  sessionId?: string;
}

const MODE_COLORS: Record<string, string> = {
  research: "text-blue-400",
  governed: "text-emerald-400",
  maintenance: "text-amber-400",
};

export function LocationHeader({ orientation, sessionId }: Props) {
  const [toggling, setToggling] = useState(false);

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

  const isActiveOverlay = orientation.lens.overlay.mode === "active_only";

  const handleOverlayToggle = async () => {
    if (!sessionId || toggling) return;
    setToggling(true);
    const newMode = isActiveOverlay ? "draft_overlay" : "active_only";
    try {
      await chatApi.sendMessage(sessionId, {
        message: `nav.set-lens overlay ${newMode}`,
      });
      queryClient.invalidateQueries({ queryKey: queryKeys.observatory.all(sessionId) });
    } catch (err) {
      console.error("Overlay toggle failed:", err);
    } finally {
      setToggling(false);
    }
  };

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
      <span className="ml-auto flex items-center gap-2">
        <span className="text-[var(--text-secondary)] text-xs">
          {enabledActions} actions
        </span>
        <button
          onClick={handleOverlayToggle}
          disabled={toggling}
          className={`px-2 py-0.5 text-[10px] font-medium rounded border ${
            isActiveOverlay
              ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-400"
              : "border-amber-500/40 bg-amber-500/10 text-amber-400"
          } hover:opacity-80 disabled:opacity-40`}
          title={
            isActiveOverlay
              ? "Showing active state. Click for draft overlay."
              : "Showing draft overlay. Click for active only."
          }
        >
          {isActiveOverlay ? "Active" : "Drafts"}
        </button>
      </span>
    </div>
  );
}

export default LocationHeader;
