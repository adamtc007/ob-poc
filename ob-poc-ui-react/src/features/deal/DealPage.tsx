/**
 * Deal Page - Main deal taxonomy visualization
 *
 * Displays the deal hierarchy with:
 * - Navigation tree on the left
 * - Detail pane on the right
 * - View mode tabs (COMMERCIAL, FINANCIAL, STATUS)
 */

import { useState, useCallback } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  Loader2,
  ArrowLeft,
  BarChart3,
  DollarSign,
  Activity,
} from "lucide-react";
import { dealApi } from "../../api/deal";
import { queryKeys } from "../../lib/query";
import { cn } from "../../lib/utils";
import { DealTaxonomyTree, DealDetailPane } from "./components";
import type { DealTaxonomyNode, DealViewMode } from "../../types/deal";

/** View mode configuration */
const VIEW_MODES: { id: DealViewMode; label: string; icon: React.ElementType }[] = [
  { id: "COMMERCIAL", label: "Commercial", icon: BarChart3 },
  { id: "FINANCIAL", label: "Financial", icon: DollarSign },
  { id: "STATUS", label: "Status", icon: Activity },
];

export function DealPage() {
  const { dealId } = useParams<{ dealId: string }>();
  const navigate = useNavigate();
  const [viewMode, setViewMode] = useState<DealViewMode>("COMMERCIAL");
  const [selectedNode, setSelectedNode] = useState<DealTaxonomyNode | null>(
    null,
  );

  // Fetch deal graph
  const {
    data: graph,
    isLoading,
    error,
  } = useQuery({
    queryKey: queryKeys.deals.graph(dealId || "", viewMode),
    queryFn: () => dealApi.getDealGraph(dealId!, viewMode),
    enabled: !!dealId,
  });

  const handleBack = useCallback(() => {
    navigate(-1);
  }, [navigate]);

  const handleSelectNode = useCallback((node: DealTaxonomyNode) => {
    setSelectedNode(node);
  }, []);

  if (!dealId) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-[var(--text-muted)]">No deal ID provided</p>
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
            Error Loading Deal
          </h2>
          <p className="mt-2 text-[var(--text-secondary)]">
            {error instanceof Error ? error.message : "Unknown error"}
          </p>
        </div>
      </div>
    );
  }

  if (!graph) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-[var(--text-muted)]">Deal not found</p>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <header className="flex items-center justify-between border-b border-[var(--border-primary)] bg-[var(--bg-secondary)] px-4 py-3">
        <div className="flex items-center gap-3">
          <button
            onClick={handleBack}
            className="p-1.5 rounded-md hover:bg-[var(--bg-hover)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
            title="Go back"
          >
            <ArrowLeft size={18} />
          </button>
          <div>
            <h1 className="text-lg font-semibold text-[var(--text-primary)]">
              {graph.deal.deal_name}
            </h1>
            <p className="text-xs text-[var(--text-muted)]">
              {graph.deal.deal_status} &middot;{" "}
              {graph.deal.client_group_name || "No client group"}
            </p>
          </div>
        </div>

        {/* View mode tabs */}
        <div className="flex items-center gap-1 bg-[var(--bg-tertiary)] rounded-lg p-1">
          {VIEW_MODES.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setViewMode(id)}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm transition-colors",
                viewMode === id
                  ? "bg-[var(--bg-primary)] text-[var(--text-primary)] shadow-sm"
                  : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]",
              )}
            >
              <Icon size={14} />
              {label}
            </button>
          ))}
        </div>
      </header>

      {/* Main content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left panel - Navigation tree */}
        <div className="w-80 flex-shrink-0 border-r border-[var(--border-primary)] bg-[var(--bg-secondary)] overflow-y-auto">
          <DealTaxonomyTree
            graph={graph}
            selectedNodeId={selectedNode?.id}
            onSelectNode={handleSelectNode}
          />
        </div>

        {/* Right panel - Detail pane */}
        <div className="flex-1 overflow-y-auto bg-[var(--bg-primary)]">
          <DealDetailPane node={selectedNode} />
        </div>
      </div>
    </div>
  );
}

export default DealPage;
