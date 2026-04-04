/**
 * Observatory Page — React shell for the egui constellation canvas.
 *
 * Two tabs:
 *   - Observe: constellation canvas (WASM) + viewport sidebar
 *   - Mission Control: health metrics dashboard
 */

import { useParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { observatoryApi } from "../../api/observatory";
import { queryClient } from "../../lib/query";
import { LocationHeader } from "./components/LocationHeader";
import { Breadcrumbs } from "./components/Breadcrumbs";
import { ViewportRenderer } from "./components/ViewportRenderer";
import { MissionControl } from "./components/MissionControl";
import { ConstellationCanvas } from "./components/ConstellationCanvas";
import { MermaidPanel } from "./components/MermaidPanel";
import type { ObservatoryAction } from "../../types/observatory";

type Tab = "observe" | "mission_control";

export function ObservatoryPage() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const [activeTab, setActiveTab] = useState<Tab>("observe");

  // Fetch orientation
  const { data: orientation } = useQuery({
    queryKey: ["observatory", "orientation", sessionId],
    queryFn: () => observatoryApi.getOrientation(sessionId!),
    enabled: !!sessionId,
    refetchInterval: 5000,
  });

  // Fetch show packet (for viewports)
  const { data: showPacket } = useQuery({
    queryKey: ["observatory", "show-packet", sessionId],
    queryFn: () => observatoryApi.getShowPacket(sessionId!),
    enabled: !!sessionId,
    refetchInterval: 5000,
  });

  // Fetch graph scene (for constellation canvas)
  const { data: graphScene } = useQuery({
    queryKey: ["observatory", "graph-scene", sessionId],
    queryFn: () => observatoryApi.getGraphScene(sessionId!),
    enabled: !!sessionId,
  });

  // Fetch navigation history (includes cursor position)
  const { data: navHistoryData } = useQuery({
    queryKey: ["observatory", "nav-history", sessionId],
    queryFn: () => observatoryApi.getNavigationHistory(sessionId!),
    enabled: !!sessionId,
  });

  // Handle action from egui canvas
  const handleCanvasAction = useCallback(
    async (action: ObservatoryAction) => {
      if (!sessionId) return;

      // Map canvas actions to navigation verbs
      let verb: string | null = null;
      let args: Record<string, unknown> = {};

      switch (action.type) {
        case "drill":
          verb = "nav.drill";
          args = {
            target_id: action.node_id,
            target_level: action.target_level,
          };
          break;
        case "semantic_zoom_out":
          verb = "nav.zoom-out";
          break;
        case "navigate_history":
          verb =
            action.direction === "back"
              ? "nav.history-back"
              : "nav.history-forward";
          break;
        case "select_node":
          verb = "nav.select";
          args = { target_id: action.node_id };
          break;
        case "invoke_verb":
          verb = action.verb_fqn;
          break;
        default:
          // Visual-only actions (pan, zoom, anchor, reset) — no server call
          return;
      }

      if (!verb) return;

      try {
        await observatoryApi.navigate(sessionId, verb, args);
        // Invalidate cached orientation + graph scene so queries refetch
        queryClient.invalidateQueries({
          queryKey: ["observatory", "orientation", sessionId],
        });
        queryClient.invalidateQueries({
          queryKey: ["observatory", "graph-scene", sessionId],
        });
        queryClient.invalidateQueries({
          queryKey: ["observatory", "nav-history", sessionId],
        });
      } catch (err) {
        console.error("Navigation failed:", err);
      }
    },
    [sessionId],
  );

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Skip when typing in an input/textarea/contenteditable
      const target = e.target as HTMLElement;
      if (
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable
      ) {
        return;
      }

      switch (e.key) {
        case "Escape":
          // Reset view — invalidate graph scene to trigger refetch
          if (sessionId) {
            queryClient.invalidateQueries({
              queryKey: ["observatory", "graph-scene", sessionId],
            });
          }
          break;
        case "Backspace":
          // Zoom out
          if (sessionId) {
            observatoryApi
              .navigate(sessionId, "nav.zoom-out", {})
              .then(() => {
                queryClient.invalidateQueries({
                  queryKey: ["observatory"],
                });
              })
              .catch((err: unknown) =>
                console.error("Zoom out failed:", err),
              );
          }
          e.preventDefault();
          break;
        case "r":
        case "R":
          // Refresh all observatory queries
          queryClient.invalidateQueries({ queryKey: ["observatory"] });
          break;
        case "m":
        case "M":
          // Toggle between observe and mission_control tabs
          setActiveTab((prev) =>
            prev === "observe" ? "mission_control" : "observe",
          );
          break;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [sessionId]);

  if (!sessionId)
    return (
      <div className="p-4 text-[var(--text-secondary)]">
        No session selected
      </div>
    );

  return (
    <div className="flex flex-col h-screen bg-[var(--bg-primary)]">
      {/* Location Header */}
      <LocationHeader orientation={orientation ?? null} sessionId={sessionId} />

      {/* Breadcrumbs */}
      <Breadcrumbs
        history={navHistoryData?.entries ?? []}
        cursor={navHistoryData?.cursor}
        sessionId={sessionId}
      />

      {/* Tab bar */}
      <div className="flex items-center gap-1 border-b border-[var(--border-primary)] px-3 py-1">
        <button
          onClick={() => setActiveTab("observe")}
          className={`px-3 py-1 text-xs font-medium rounded ${
            activeTab === "observe"
              ? "bg-[var(--bg-active)] text-[var(--text-primary)]"
              : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
          }`}
        >
          Observe
        </button>
        <button
          onClick={() => setActiveTab("mission_control")}
          className={`px-3 py-1 text-xs font-medium rounded ${
            activeTab === "mission_control"
              ? "bg-[var(--bg-active)] text-[var(--text-primary)]"
              : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
          }`}
        >
          Mission Control
        </button>
      </div>

      {/* Main content */}
      {activeTab === "observe" ? (
        <div className="flex flex-col flex-1 min-h-0">
          {orientation?.view_level === "surface" ? (
            <>
              {/* Surface level: canvas minimized, viewport expanded */}
              <div className="h-32 min-h-[8rem] border-b border-[var(--border-primary)] flex items-center justify-center">
                <ConstellationCanvas
                  graphScene={graphScene ?? null}
                  viewLevel={orientation.view_level}
                  onAction={handleCanvasAction}
                />
                <div className="absolute text-xs text-[var(--text-secondary)] pointer-events-none">
                  Surface level — see viewport panels
                </div>
              </div>
              <div className="flex-1 overflow-y-auto">
                <ViewportRenderer
                  showPacket={showPacket}
                  orientation={orientation ?? null}
                  sessionId={sessionId}
                />
                <MermaidPanel sessionId={sessionId} />
              </div>
            </>
          ) : (
            <div className="flex flex-1 min-h-0">
              {/* Constellation canvas (egui WASM) */}
              <div className="flex-1 min-w-0">
                <ConstellationCanvas
                  graphScene={graphScene ?? null}
                  viewLevel={orientation?.view_level ?? "system"}
                  onAction={handleCanvasAction}
                />
              </div>

              {/* Viewport sidebar */}
              <div className="w-80 border-l border-[var(--border-primary)] overflow-y-auto">
                <ViewportRenderer
                  showPacket={showPacket}
                  orientation={orientation ?? null}
                  sessionId={sessionId}
                />
                <MermaidPanel sessionId={sessionId} />
              </div>
            </div>
          )}
        </div>
      ) : (
        <MissionControl sessionId={sessionId} />
      )}
    </div>
  );
}

export default ObservatoryPage;
