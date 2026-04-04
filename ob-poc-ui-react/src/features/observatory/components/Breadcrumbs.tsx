/**
 * Breadcrumbs — navigation history trail with back/forward controls.
 *
 * Each entry is clickable (navigates to that entry's focus).
 * Back/forward buttons navigate through history via server round-trip.
 */

import { useCallback } from "react";
import { observatoryApi } from "../../../api/observatory";
import { queryClient } from "../../../lib/query";
import type { OrientationContract } from "../../../types/observatory";

interface Props {
  history: OrientationContract[];
  cursor?: number;
  sessionId: string;
}

export function Breadcrumbs({ history, cursor, sessionId }: Props) {
  const currentIndex = cursor ?? history.length - 1;
  const canGoBack = currentIndex > 0;
  const canGoForward = currentIndex < history.length - 1;

  const invalidateQueries = useCallback(() => {
    queryClient.invalidateQueries({
      queryKey: ["observatory", "orientation", sessionId],
    });
    queryClient.invalidateQueries({
      queryKey: ["observatory", "graph-scene", sessionId],
    });
    queryClient.invalidateQueries({
      queryKey: ["observatory", "nav-history", sessionId],
    });
  }, [sessionId]);

  const handleBack = useCallback(async () => {
    if (!canGoBack) return;
    try {
      await observatoryApi.navigate(sessionId, "nav.history-back", {});
      invalidateQueries();
    } catch (err) {
      console.error("History back failed:", err);
    }
  }, [sessionId, canGoBack, invalidateQueries]);

  const handleForward = useCallback(async () => {
    if (!canGoForward) return;
    try {
      await observatoryApi.navigate(sessionId, "nav.history-forward", {});
      invalidateQueries();
    } catch (err) {
      console.error("History forward failed:", err);
    }
  }, [sessionId, canGoForward, invalidateQueries]);

  const handleEntryClick = useCallback(
    async (entry: OrientationContract) => {
      // Navigate to the focus of the clicked breadcrumb entry
      try {
        await observatoryApi.navigate(sessionId, "nav.drill", {
          target_id: entry.focus_identity.canonical_id,
          target_level: entry.view_level,
        });
        invalidateQueries();
      } catch (err) {
        console.error("Breadcrumb navigation failed:", err);
      }
    },
    [sessionId, invalidateQueries],
  );

  if (history.length === 0) return null;

  return (
    <div className="flex items-center gap-1 px-4 py-1 text-xs border-b border-[var(--border-primary)] overflow-x-auto">
      {/* Back / Forward buttons */}
      <button
        onClick={handleBack}
        disabled={!canGoBack}
        className={`px-1 py-0.5 rounded text-sm ${
          canGoBack
            ? "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] cursor-pointer"
            : "text-[var(--text-muted)] cursor-not-allowed opacity-40"
        }`}
        title="Back"
      >
        &larr;
      </button>
      <button
        onClick={handleForward}
        disabled={!canGoForward}
        className={`px-1 py-0.5 rounded text-sm ${
          canGoForward
            ? "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] cursor-pointer"
            : "text-[var(--text-muted)] cursor-not-allowed opacity-40"
        }`}
        title="Forward"
      >
        &rarr;
      </button>

      <span className="mx-1 text-[var(--border-primary)]">|</span>

      {/* Breadcrumb entries */}
      {history.map((entry, i) => (
        <span key={i} className="flex items-center gap-1 shrink-0">
          {i > 0 && (
            <span className="text-[var(--text-muted)]">&rsaquo;</span>
          )}
          <button
            onClick={() => handleEntryClick(entry)}
            className={`px-1.5 py-0.5 rounded ${
              i === currentIndex
                ? "text-[var(--text-primary)] font-medium bg-[var(--bg-active)]"
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
