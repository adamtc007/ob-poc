/**
 * Inspector Page - Main inspector UI route
 *
 * Displays the projection tree on the left and detail pane on the right.
 */

import { useParams } from "react-router-dom";
import { useQuery, useMutation } from "@tanstack/react-query";
import { projectionsApi } from "../../api/projections";
import { queryKeys, queryClient } from "../../lib/query";
import { useInspectorStore } from "../../stores/inspector";
import { useEffect, useCallback } from "react";
import { Loader2, Search, Settings2 } from "lucide-react";
import {
  NavigationTree,
  Breadcrumbs,
  DetailPane,
  PolicyControls,
} from "./components";
import { cn } from "../../lib/utils";
import { useState } from "react";

export function InspectorPage() {
  const { projectionId } = useParams<{ projectionId?: string }>();
  const [showPolicyPanel, setShowPolicyPanel] = useState(false);
  const {
    setProjection,
    setLoading,
    setError,
    projection,
    policy,
    searchQuery,
    setSearchQuery,
  } = useInspectorStore();

  // Fetch projection if ID is provided
  const { data, isLoading, error } = useQuery({
    queryKey: queryKeys.projections.detail(projectionId || ""),
    queryFn: () => projectionsApi.get(projectionId!),
    enabled: !!projectionId,
  });

  // Regenerate mutation
  const regenerateMutation = useMutation({
    mutationFn: () => {
      if (!projection) throw new Error("No projection to regenerate");
      return projectionsApi.generate({
        snapshot_id: projectionId!, // Use projection ID as snapshot ID for now
        policy,
      });
    },
    onSuccess: (newProjection) => {
      setProjection(newProjection);
      queryClient.invalidateQueries({ queryKey: queryKeys.projections.all });
    },
  });

  // Update store when data changes
  useEffect(() => {
    setLoading(isLoading);
    if (error) {
      setError(
        error instanceof Error ? error.message : "Failed to load projection",
      );
    } else if (data) {
      setProjection(data);
    }
  }, [data, isLoading, error, setProjection, setLoading, setError]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // "/" to focus search
      if (e.key === "/" && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        document.getElementById("inspector-search")?.focus();
      }
      // 1-4 for LOD levels
      if (["1", "2", "3", "4"].includes(e.key) && !e.ctrlKey && !e.metaKey) {
        const lod = parseInt(e.key, 10) - 1;
        if (lod >= 0 && lod <= 3) {
          useInspectorStore.getState().setLod(lod as 0 | 1 | 2 | 3);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const handleRegenerate = useCallback(() => {
    regenerateMutation.mutate();
  }, [regenerateMutation]);

  if (!projectionId) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-center">
          <h2 className="text-xl font-semibold text-[var(--text-primary)]">
            No Projection Selected
          </h2>
          <p className="mt-2 text-[var(--text-secondary)]">
            Select a projection from the chat or generate one to start
            inspecting.
          </p>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-[var(--accent-blue)]" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-center">
          <h2 className="text-xl font-semibold text-[var(--accent-red)]">
            Error Loading Projection
          </h2>
          <p className="mt-2 text-[var(--text-secondary)]">
            {error instanceof Error ? error.message : "Unknown error"}
          </p>
        </div>
      </div>
    );
  }

  if (!projection) {
    return null;
  }

  return (
    <div className="flex h-full">
      {/* Left panel - Navigation tree */}
      <div className="w-80 flex-shrink-0 flex flex-col border-r border-[var(--border-primary)] bg-[var(--bg-secondary)]">
        {/* Header with search */}
        <div className="flex items-center gap-2 border-b border-[var(--border-primary)] px-3 py-2">
          <div className="relative flex-1">
            <Search
              size={14}
              className="absolute left-2 top-1/2 -translate-y-1/2 text-[var(--text-muted)]"
            />
            <input
              id="inspector-search"
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search... (/)"
              className="w-full rounded bg-[var(--bg-tertiary)] pl-7 pr-2 py-1.5 text-sm text-[var(--text-primary)] placeholder-[var(--text-muted)] focus:outline-none focus:ring-1 focus:ring-[var(--accent-blue)]"
            />
          </div>
          <button
            onClick={() => setShowPolicyPanel(!showPolicyPanel)}
            className={cn(
              "rounded p-1.5 transition-colors",
              showPolicyPanel
                ? "bg-[var(--accent-blue)]/20 text-[var(--accent-blue)]"
                : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]",
            )}
            title="Policy controls"
          >
            <Settings2 size={16} />
          </button>
        </div>

        {/* Policy controls (collapsible) */}
        {showPolicyPanel && (
          <div className="border-b border-[var(--border-primary)] p-3">
            <PolicyControls onRegenerate={handleRegenerate} />
            {regenerateMutation.isPending && (
              <div className="mt-2 flex items-center justify-center gap-2 text-sm text-[var(--text-muted)]">
                <Loader2 size={14} className="animate-spin" />
                Regenerating...
              </div>
            )}
          </div>
        )}

        {/* Tree */}
        <div className="flex-1 overflow-auto">
          <NavigationTree />
        </div>
      </div>

      {/* Right panel - Detail pane */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Breadcrumbs header */}
        <div className="flex items-center border-b border-[var(--border-primary)] px-4 py-2 bg-[var(--bg-secondary)]">
          <Breadcrumbs />
        </div>

        {/* Detail content */}
        <div className="flex-1 overflow-auto bg-[var(--bg-primary)]">
          <DetailPane />
        </div>
      </div>
    </div>
  );
}

export default InspectorPage;
