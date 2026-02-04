/**
 * Viewport Page - Pop-out window for full-screen CBU/Entity visualization
 *
 * This page is designed to be opened in a separate browser window,
 * showing the session's scope (loaded CBUs) in a detailed graph/list view.
 */

import { useParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  Building2,
  ChevronRight,
  ExternalLink,
  Loader2,
  MapPin,
  RefreshCw,
  User,
  Users,
  X,
} from "lucide-react";
import { useState } from "react";
import { scopeApi, type CbuSummary, type EntitySummary } from "../../api/scope";
import { queryKeys } from "../../lib/query";

function CbuCard({
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
      className={`w-full p-4 rounded-lg border text-left transition-all ${
        isSelected
          ? "border-[var(--accent-blue)] bg-[var(--accent-blue)]/10 ring-2 ring-[var(--accent-blue)]/30"
          : "border-[var(--border-primary)] bg-[var(--bg-secondary)] hover:border-[var(--accent-blue)]/50"
      }`}
    >
      <div className="flex items-start gap-3">
        <Building2
          size={20}
          className={`flex-shrink-0 mt-0.5 ${isSelected ? "text-[var(--accent-blue)]" : "text-[var(--text-muted)]"}`}
        />
        <div className="flex-1 min-w-0">
          <div className="font-medium text-[var(--text-primary)] truncate">
            {cbu.name}
          </div>
          <div className="flex items-center gap-3 mt-1 text-sm text-[var(--text-muted)]">
            {cbu.kind && <span>{cbu.kind}</span>}
            {cbu.jurisdiction && (
              <span className="flex items-center gap-1">
                <MapPin size={12} />
                {cbu.jurisdiction}
              </span>
            )}
          </div>
        </div>
        <ChevronRight
          size={16}
          className={`flex-shrink-0 ${isSelected ? "text-[var(--accent-blue)]" : "text-[var(--text-muted)]"}`}
        />
      </div>
    </button>
  );
}

function EntityCard({ entity }: { entity: EntitySummary }) {
  return (
    <div className="p-3 rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)]">
      <div className="flex items-start gap-3">
        {entity.entityType === "person" ? (
          <User size={18} className="text-[var(--accent-green)] flex-shrink-0 mt-0.5" />
        ) : (
          <Users size={18} className="text-[var(--accent-purple)] flex-shrink-0 mt-0.5" />
        )}
        <div className="flex-1 min-w-0">
          <div className="font-medium text-[var(--text-primary)]">{entity.name}</div>
          <div className="flex items-center gap-2 mt-1 text-sm text-[var(--text-muted)]">
            {entity.entityType && <span>{entity.entityType}</span>}
            {entity.role && (
              <span className="px-2 py-0.5 bg-[var(--bg-tertiary)] rounded text-xs">
                {entity.role}
              </span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function CbuDetailPanel({ cbu }: { cbu: CbuSummary }) {
  const { data, isLoading, error } = useQuery({
    queryKey: ["cbu-graph", cbu.id],
    queryFn: () => scopeApi.getCbuGraph(cbu.id),
  });

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-4 border-b border-[var(--border-primary)]">
        <div className="flex items-center gap-3">
          <Building2 size={24} className="text-[var(--accent-blue)]" />
          <div>
            <h2 className="text-lg font-semibold text-[var(--text-primary)]">
              {cbu.name}
            </h2>
            <div className="flex items-center gap-3 text-sm text-[var(--text-muted)]">
              {cbu.kind && <span>{cbu.kind}</span>}
              {cbu.jurisdiction && (
                <span className="flex items-center gap-1">
                  <MapPin size={12} />
                  {cbu.jurisdiction}
                </span>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-4">
        {isLoading ? (
          <div className="flex items-center justify-center h-32">
            <Loader2 size={24} className="animate-spin text-[var(--text-muted)]" />
          </div>
        ) : error ? (
          <div className="text-[var(--accent-red)] text-center py-8">
            {error instanceof Error ? error.message : "Failed to load CBU details"}
          </div>
        ) : (
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="font-medium text-[var(--text-primary)]">
                Entities ({data?.entities.length ?? 0})
              </h3>
              <div className="text-sm text-[var(--text-muted)]">
                {data?.nodeCount} nodes, {data?.edgeCount} edges
              </div>
            </div>
            <div className="grid gap-2">
              {data?.entities.map((entity) => (
                <EntityCard key={entity.id} entity={entity} />
              ))}
              {data?.entities.length === 0 && (
                <div className="text-center py-8 text-[var(--text-muted)]">
                  No entities in this CBU
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export function ViewportPage() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const [selectedCbu, setSelectedCbu] = useState<CbuSummary | null>(null);

  const { data, isLoading, error, refetch, isRefetching } = useQuery({
    queryKey: queryKeys.scope(sessionId || ""),
    queryFn: () => scopeApi.getScope(sessionId!),
    enabled: !!sessionId,
    refetchInterval: 5000,
  });

  const handleClose = () => {
    window.close();
  };

  if (!sessionId) {
    return (
      <div className="h-screen flex items-center justify-center bg-[var(--bg-primary)]">
        <div className="text-center">
          <h1 className="text-xl font-semibold text-[var(--text-primary)]">
            No Session
          </h1>
          <p className="mt-2 text-[var(--text-muted)]">
            Open this viewport from a chat session.
          </p>
        </div>
      </div>
    );
  }

  const cbus = data?.cbus ?? [];
  const cbuCount = data?.cbuCount ?? 0;

  return (
    <div className="h-screen flex flex-col bg-[var(--bg-primary)]">
      {/* Header */}
      <header className="flex items-center justify-between px-4 py-3 border-b border-[var(--border-primary)] bg-[var(--bg-secondary)]">
        <div className="flex items-center gap-3">
          <ExternalLink size={20} className="text-[var(--accent-blue)]" />
          <div>
            <h1 className="font-semibold text-[var(--text-primary)]">
              Session Viewport
            </h1>
            <p className="text-sm text-[var(--text-muted)]">
              {sessionId.slice(0, 8)}... &middot; {cbuCount} CBUs in scope
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => refetch()}
            disabled={isRefetching}
            className="p-2 hover:bg-[var(--bg-tertiary)] rounded-lg transition-colors"
            title="Refresh"
          >
            <RefreshCw
              size={18}
              className={`text-[var(--text-muted)] ${isRefetching ? "animate-spin" : ""}`}
            />
          </button>
          <button
            onClick={handleClose}
            className="p-2 hover:bg-[var(--bg-tertiary)] rounded-lg transition-colors"
            title="Close window"
          >
            <X size={18} className="text-[var(--text-muted)]" />
          </button>
        </div>
      </header>

      {/* Main content */}
      <div className="flex-1 flex overflow-hidden">
        {/* CBU list (left panel) */}
        <div className="w-80 flex-shrink-0 border-r border-[var(--border-primary)] overflow-auto p-4">
          <h2 className="font-medium text-[var(--text-primary)] mb-3">
            Structures ({cbuCount})
          </h2>

          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 size={24} className="animate-spin text-[var(--text-muted)]" />
            </div>
          ) : error ? (
            <div className="text-[var(--accent-red)] text-center py-8">
              {error instanceof Error ? error.message : "Failed to load scope"}
            </div>
          ) : data?.error ? (
            <div className="text-[var(--text-muted)] text-center py-8">
              {data.error}
            </div>
          ) : cbus.length === 0 ? (
            <div className="text-[var(--text-muted)] text-center py-8">
              <p>No CBUs loaded.</p>
              <p className="text-sm mt-2">
                Use the chat to load a client book or CBU set.
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              {cbus.map((cbu) => (
                <CbuCard
                  key={cbu.id}
                  cbu={cbu}
                  isSelected={selectedCbu?.id === cbu.id}
                  onClick={() => setSelectedCbu(cbu)}
                />
              ))}
            </div>
          )}
        </div>

        {/* Detail panel (right) */}
        <div className="flex-1 overflow-hidden">
          {selectedCbu ? (
            <CbuDetailPanel cbu={selectedCbu} />
          ) : (
            <div className="h-full flex items-center justify-center text-[var(--text-muted)]">
              <div className="text-center">
                <Building2 size={48} className="mx-auto mb-4 opacity-30" />
                <p>Select a structure to view details</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default ViewportPage;
