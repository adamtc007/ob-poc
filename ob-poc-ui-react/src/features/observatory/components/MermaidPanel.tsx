/**
 * MermaidPanel — collapsible panel for fetching and displaying Mermaid diagrams.
 *
 * Supports diagram types: ERD, Verb Flow, Domain Map, Discovery Map.
 * Currently renders raw Mermaid syntax in a <pre> block.
 * TODO: Add mermaid.js rendering for interactive SVG diagrams.
 */

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { observatoryApi } from "../../../api/observatory";
import { queryKeys } from "../../../lib/query";

interface Props {
  sessionId: string;
}

type DiagramType = "erd" | "verb_flow" | "domain_map" | "discovery_map";

const DIAGRAM_LABELS: Record<DiagramType, string> = {
  erd: "ERD",
  verb_flow: "Verb Flow",
  domain_map: "Domain Map",
  discovery_map: "Discovery Map",
};

export function MermaidPanel({ sessionId }: Props) {
  const [collapsed, setCollapsed] = useState(false);
  const [activeDiagram, setActiveDiagram] = useState<DiagramType>("erd");

  const { data, isLoading, error } = useQuery({
    queryKey: queryKeys.observatory.diagram(sessionId, activeDiagram),
    queryFn: () => observatoryApi.getDiagram(sessionId, activeDiagram),
    enabled: !collapsed,
  });

  return (
    <div className="border-t border-[var(--border-primary)]">
      <button
        onClick={() => setCollapsed((c) => !c)}
        className="flex items-center gap-2 w-full px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] hover:bg-[var(--bg-hover)]"
      >
        <span
          className={`transition-transform ${collapsed ? "" : "rotate-90"}`}
        >
          &#9654;
        </span>
        Diagrams
      </button>

      {!collapsed && (
        <div className="px-3 pb-3">
          {/* Diagram type selector */}
          <div className="flex gap-1 mb-2">
            {(Object.keys(DIAGRAM_LABELS) as DiagramType[]).map((type) => (
              <button
                key={type}
                onClick={() => setActiveDiagram(type)}
                className={`px-2 py-0.5 text-[10px] rounded font-medium ${
                  activeDiagram === type
                    ? "bg-[var(--bg-active)] text-[var(--text-primary)]"
                    : "text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]"
                }`}
              >
                {DIAGRAM_LABELS[type]}
              </button>
            ))}
          </div>

          {/* Diagram content */}
          {isLoading && (
            <div className="text-[10px] text-[var(--text-muted)]">
              Loading diagram...
            </div>
          )}
          {error && (
            <div className="text-[10px] text-red-400">
              Failed to load diagram
            </div>
          )}
          {data?.mermaid && (
            <pre className="whitespace-pre-wrap text-[10px] font-mono bg-[var(--bg-secondary)] rounded border border-[var(--border-secondary)] p-2 max-h-64 overflow-y-auto text-[var(--text-secondary)]">
              {data.mermaid}
            </pre>
          )}
          {data && !data.mermaid && (
            <div className="text-[10px] text-[var(--text-muted)]">
              No diagram available
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default MermaidPanel;
