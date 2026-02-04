/**
 * Scope Panel - Shows loaded CBUs in session with drill-down
 */

import { useQuery } from "@tanstack/react-query";
import {
  ArrowLeft,
  Building2,
  ChevronDown,
  ChevronRight,
  Loader2,
  MapPin,
  User,
  Users,
} from "lucide-react";
import { useState } from "react";
import {
  scopeApi,
  type CbuSummary,
  type EntitySummary,
} from "../../../api/scope";
import { queryKeys } from "../../../lib/query";

interface ScopePanelProps {
  sessionId: string | undefined;
  className?: string;
}

function CbuItem({
  cbu,
  isSelected,
  onClick,
}: {
  cbu: CbuSummary;
  isSelected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={`w-full flex items-center gap-2 px-3 py-2 rounded text-sm text-left transition-colors ${
        isSelected
          ? "bg-[var(--accent-blue)] text-white"
          : "hover:bg-[var(--bg-tertiary)]"
      }`}
    >
      <Building2
        size={14}
        className={`flex-shrink-0 ${isSelected ? "text-white" : "text-[var(--accent-blue)]"}`}
      />
      <div className="flex-1 min-w-0">
        <div className="truncate">{cbu.name}</div>
        <div
          className={`flex items-center gap-2 text-xs ${isSelected ? "text-white/80" : "text-[var(--text-muted)]"}`}
        >
          {cbu.kind && <span className="truncate">{cbu.kind}</span>}
          {cbu.jurisdiction && (
            <span className="flex items-center gap-0.5">
              <MapPin size={10} />
              {cbu.jurisdiction}
            </span>
          )}
        </div>
      </div>
      <ChevronRight
        size={14}
        className={isSelected ? "text-white" : "text-[var(--text-muted)]"}
      />
    </button>
  );
}

function EntityItem({ entity }: { entity: EntitySummary }) {
  return (
    <div className="flex items-center gap-2 px-3 py-2 hover:bg-[var(--bg-tertiary)] rounded text-sm">
      {entity.entityType === "person" ? (
        <User size={14} className="text-[var(--accent-green)] flex-shrink-0" />
      ) : (
        <Users
          size={14}
          className="text-[var(--accent-purple)] flex-shrink-0"
        />
      )}
      <div className="flex-1 min-w-0">
        <div className="truncate text-[var(--text-primary)]">{entity.name}</div>
        <div className="flex items-center gap-2 text-xs text-[var(--text-muted)]">
          {entity.entityType && <span>{entity.entityType}</span>}
          {entity.role && (
            <span className="px-1 py-0.5 bg-[var(--bg-tertiary)] rounded">
              {entity.role}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

function CbuDetailView({
  cbu,
  onBack,
}: {
  cbu: CbuSummary;
  onBack: () => void;
}) {
  const { data, isLoading, error } = useQuery({
    queryKey: ["cbu-graph", cbu.id],
    queryFn: () => scopeApi.getCbuGraph(cbu.id),
  });

  return (
    <div className="flex flex-col h-full">
      {/* Header with back button */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border-primary)]">
        <button
          onClick={onBack}
          className="p-1 hover:bg-[var(--bg-tertiary)] rounded"
        >
          <ArrowLeft size={16} className="text-[var(--text-muted)]" />
        </button>
        <div className="flex-1 min-w-0">
          <div className="font-medium text-sm truncate text-[var(--text-primary)]">
            {cbu.name}
          </div>
          <div className="text-xs text-[var(--text-muted)]">
            {cbu.kind} {cbu.jurisdiction && `• ${cbu.jurisdiction}`}
          </div>
        </div>
      </div>

      {/* Entity list */}
      <div className="flex-1 overflow-auto p-2">
        {isLoading ? (
          <div className="flex items-center justify-center py-8">
            <Loader2
              size={20}
              className="animate-spin text-[var(--text-muted)]"
            />
          </div>
        ) : error ? (
          <div className="text-sm text-[var(--accent-red)] p-2 text-center">
            {error instanceof Error ? error.message : "Failed to load CBU"}
          </div>
        ) : !data?.entities.length ? (
          <div className="text-sm text-[var(--text-muted)] p-2 text-center">
            No entities in this CBU
          </div>
        ) : (
          <div className="space-y-1">
            <div className="text-xs font-medium text-[var(--text-muted)] px-3 py-1">
              Entities ({data.entities.length})
            </div>
            {data.entities.map((entity) => (
              <EntityItem key={entity.id} entity={entity} />
            ))}
          </div>
        )}
      </div>

      {/* Stats footer */}
      {data && (
        <div className="px-3 py-2 border-t border-[var(--border-primary)] text-xs text-[var(--text-muted)]">
          {data.nodeCount} nodes • {data.edgeCount} edges
        </div>
      )}
    </div>
  );
}

export function ScopePanel({ sessionId, className = "" }: ScopePanelProps) {
  const [isExpanded, setIsExpanded] = useState(true);
  const [selectedCbu, setSelectedCbu] = useState<CbuSummary | null>(null);

  const { data, isLoading, error } = useQuery({
    queryKey: queryKeys.scope(sessionId || ""),
    queryFn: () => scopeApi.getScope(sessionId!),
    enabled: !!sessionId,
    refetchInterval: 5000, // Refresh every 5 seconds to catch scope changes
  });

  // Don't render if no session
  if (!sessionId) {
    return null;
  }

  const cbuCount = data?.cbuCount ?? 0;
  const cbus = data?.cbus ?? [];

  // Show detail view if a CBU is selected
  if (selectedCbu) {
    return (
      <div
        className={`border-l border-[var(--border-primary)] bg-[var(--bg-secondary)] flex flex-col ${className}`}
      >
        <CbuDetailView cbu={selectedCbu} onBack={() => setSelectedCbu(null)} />
      </div>
    );
  }

  return (
    <div
      className={`border-l border-[var(--border-primary)] bg-[var(--bg-secondary)] ${className}`}
    >
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between px-3 py-2 hover:bg-[var(--bg-tertiary)] border-b border-[var(--border-primary)]"
      >
        <div className="flex items-center gap-2">
          {isExpanded ? (
            <ChevronDown size={16} className="text-[var(--text-muted)]" />
          ) : (
            <ChevronRight size={16} className="text-[var(--text-muted)]" />
          )}
          <span className="text-sm font-medium text-[var(--text-primary)]">
            Scope
          </span>
          {cbuCount > 0 && (
            <span className="text-xs px-1.5 py-0.5 rounded bg-[var(--accent-blue)] text-white">
              {cbuCount}
            </span>
          )}
        </div>
        {isLoading && (
          <Loader2
            size={14}
            className="animate-spin text-[var(--text-muted)]"
          />
        )}
      </button>

      {/* Content */}
      {isExpanded && (
        <div className="p-2 max-h-[50vh] overflow-auto">
          {error ? (
            <div className="text-sm text-[var(--accent-red)] p-2">
              {error instanceof Error ? error.message : "Failed to load scope"}
            </div>
          ) : data?.error ? (
            <div className="text-sm text-[var(--text-muted)] p-2 text-center">
              {data.error}
            </div>
          ) : cbus.length === 0 ? (
            <div className="text-sm text-[var(--text-muted)] p-2 text-center">
              No CBUs loaded.
              <br />
              <span className="text-xs">
                Try: "load the allianz book" or "session.load-cluster"
              </span>
            </div>
          ) : (
            <div className="space-y-1">
              {cbus.slice(0, 50).map((cbu) => (
                <CbuItem
                  key={cbu.id}
                  cbu={cbu}
                  isSelected={false}
                  onClick={() => setSelectedCbu(cbu)}
                />
              ))}
              {cbus.length > 50 && (
                <div className="text-xs text-[var(--text-muted)] text-center py-2">
                  + {cbus.length - 50} more CBUs
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default ScopePanel;
