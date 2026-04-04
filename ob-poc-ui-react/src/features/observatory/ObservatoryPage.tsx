/**
 * Observatory Page — React shell for the egui constellation canvas.
 *
 * Two tabs:
 *   - Observe: constellation canvas (WASM) + viewport sidebar
 *   - Mission Control: health metrics dashboard
 */

import { useParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { observatoryApi } from "../../api/observatory";
import { LocationHeader } from "./components/LocationHeader";
import { Breadcrumbs } from "./components/Breadcrumbs";
import { ViewportRenderer } from "./components/ViewportRenderer";
import { MissionControl } from "./components/MissionControl";
import { ConstellationCanvas } from "./components/ConstellationCanvas";
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

  // Fetch navigation history
  const { data: navHistory } = useQuery({
    queryKey: ["observatory", "nav-history", sessionId],
    queryFn: () => observatoryApi.getNavigationHistory(sessionId!),
    enabled: !!sessionId,
  });

  // Handle action from egui canvas
  const handleCanvasAction = useCallback((action: ObservatoryAction) => {
    console.log("Canvas action:", action);
    // Semantic actions trigger server calls
    if (action.type === "drill" || action.type === "semantic_zoom_out") {
      // TODO: POST navigation verb, then refetch orientation + scene
    }
  }, []);

  if (!sessionId)
    return (
      <div className="p-4 text-[var(--text-secondary)]">
        No session selected
      </div>
    );

  return (
    <div className="flex flex-col h-screen bg-[var(--bg-primary)]">
      {/* Location Header */}
      <LocationHeader orientation={orientation ?? null} />

      {/* Breadcrumbs */}
      <Breadcrumbs history={navHistory ?? []} />

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
            />
          </div>
        </div>
      ) : (
        <MissionControl sessionId={sessionId} />
      )}
    </div>
  );
}

export default ObservatoryPage;
