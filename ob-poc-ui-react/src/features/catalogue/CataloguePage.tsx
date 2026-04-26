/**
 * Catalogue Workspace Page — Tranche 3 Phase 3.D / Observatory Phase 8 (2026-04-27).
 *
 * Read-only view of the Catalogue workspace state:
 *   - Pending proposals (DRAFT + STAGED) with status + proposer.
 *   - Single-proposal detail with the proposed declaration JSON.
 *   - Live tier-distribution heatmap (Phase 2.G.2 data).
 *
 * Production authorship goes through Sage / REPL → catalogue.* verbs;
 * this page surfaces what's in flight + the live tier landscape so authors
 * can see the impact of their work without leaving the Observatory.
 *
 * Full canvas integration (egui WASM with diff-preview component, ABAC
 * two-eye visualization, interactive heatmap brush) is the egui Phase 8
 * follow-on; this React panel ships the minimal viable Phase 8 surface.
 */

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  catalogueApi,
  type ProposalListFilter,
  type ProposalSummary,
} from "../../api/catalogue";

const FILTER_OPTIONS: { value: ProposalListFilter; label: string }[] = [
  { value: "pending", label: "Pending" },
  { value: "committed", label: "Committed" },
  { value: "rolled_back", label: "Rolled back" },
  { value: "all", label: "All" },
];

export function CataloguePage() {
  const [filter, setFilter] = useState<ProposalListFilter>("pending");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const proposalsQuery = useQuery({
    queryKey: ["catalogue", "proposals", filter],
    queryFn: () => catalogueApi.listProposals(filter),
    refetchInterval: 5_000, // live refresh every 5s
  });

  const detailQuery = useQuery({
    queryKey: ["catalogue", "proposal", selectedId],
    queryFn: () =>
      selectedId ? catalogueApi.getProposal(selectedId) : Promise.resolve(null),
    enabled: !!selectedId,
  });

  const tierQuery = useQuery({
    queryKey: ["catalogue", "tier-distribution"],
    queryFn: () => catalogueApi.getTierDistribution(),
    refetchInterval: 30_000, // refresh every 30s
  });

  return (
    <div className="flex h-full">
      {/* Left: proposals list */}
      <div className="w-1/3 border-r border-gray-200 overflow-y-auto p-4">
        <h2 className="text-lg font-semibold mb-2">Catalogue Proposals</h2>
        <div className="flex gap-1 mb-3">
          {FILTER_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              onClick={() => setFilter(opt.value)}
              className={`px-2 py-1 text-xs rounded ${
                filter === opt.value
                  ? "bg-blue-600 text-white"
                  : "bg-gray-100 text-gray-700"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
        {proposalsQuery.isLoading && (
          <div className="text-sm text-gray-500">Loading…</div>
        )}
        {proposalsQuery.data?.length === 0 && (
          <div className="text-sm text-gray-500 italic">
            No proposals matching '{filter}'.
          </div>
        )}
        <ul className="space-y-1">
          {proposalsQuery.data?.map((p) => (
            <ProposalRow
              key={p.proposal_id}
              proposal={p}
              isSelected={p.proposal_id === selectedId}
              onSelect={() => setSelectedId(p.proposal_id)}
            />
          ))}
        </ul>
      </div>

      {/* Middle: proposal detail */}
      <div className="flex-1 overflow-y-auto p-4 border-r border-gray-200">
        <h2 className="text-lg font-semibold mb-2">Proposal Detail</h2>
        {!selectedId && (
          <div className="text-sm text-gray-500 italic">
            Select a proposal on the left to see its declaration.
          </div>
        )}
        {detailQuery.isLoading && (
          <div className="text-sm text-gray-500">Loading…</div>
        )}
        {detailQuery.data && (
          <div className="space-y-3 text-sm">
            <div>
              <span className="text-gray-500">verb_fqn:</span>{" "}
              <code className="bg-gray-100 px-1 rounded">
                {detailQuery.data.verb_fqn}
              </code>
            </div>
            <div>
              <span className="text-gray-500">status:</span>{" "}
              <StatusPill status={detailQuery.data.status} />
            </div>
            <div>
              <span className="text-gray-500">proposed_by:</span>{" "}
              {detailQuery.data.proposed_by}
            </div>
            {detailQuery.data.committed_by && (
              <div>
                <span className="text-gray-500">committed_by:</span>{" "}
                {detailQuery.data.committed_by}
                {detailQuery.data.committed_by ===
                  detailQuery.data.proposed_by && (
                  <span className="ml-2 text-red-600 font-semibold">
                    ⚠ TWO-EYE RULE VIOLATION
                  </span>
                )}
              </div>
            )}
            {detailQuery.data.rationale && (
              <div>
                <span className="text-gray-500">rationale:</span>{" "}
                {detailQuery.data.rationale}
              </div>
            )}
            {detailQuery.data.rolled_back_reason && (
              <div>
                <span className="text-gray-500">rollback reason:</span>{" "}
                {detailQuery.data.rolled_back_reason}
              </div>
            )}
            <div>
              <h3 className="font-semibold mb-1">Proposed declaration:</h3>
              <pre className="bg-gray-50 border border-gray-200 rounded p-2 overflow-x-auto text-xs">
                {JSON.stringify(detailQuery.data.proposed_declaration, null, 2)}
              </pre>
            </div>
          </div>
        )}
      </div>

      {/* Right: tier distribution */}
      <div className="w-1/3 overflow-y-auto p-4">
        <h2 className="text-lg font-semibold mb-2">Tier Distribution</h2>
        <p className="text-xs text-gray-500 mb-2">
          From <code>catalogue_committed_verbs</code> (Stage 4 source-of-truth).
        </p>
        {tierQuery.isLoading && (
          <div className="text-sm text-gray-500">Loading…</div>
        )}
        {tierQuery.data && tierQuery.data.total_verbs === 0 && (
          <div className="text-sm text-gray-500 italic">
            No committed verbs yet. Catalogue is still YAML-primary.
          </div>
        )}
        {tierQuery.data && tierQuery.data.total_verbs > 0 && (
          <>
            <div className="mb-3 text-sm">
              <div className="font-semibold">
                Total: {tierQuery.data.total_verbs} verbs
              </div>
            </div>
            <div className="space-y-1 mb-4">
              {Object.entries(tierQuery.data.by_tier).map(([tier, n]) => (
                <TierBar
                  key={tier}
                  tier={tier}
                  count={n}
                  total={tierQuery.data!.total_verbs}
                />
              ))}
            </div>
            <h3 className="font-semibold text-sm mb-1">By domain × tier</h3>
            <table className="text-xs w-full">
              <thead>
                <tr className="text-left text-gray-500 border-b">
                  <th className="py-1">domain</th>
                  <th className="py-1">benign</th>
                  <th className="py-1">review</th>
                  <th className="py-1">confirm</th>
                  <th className="py-1">auth</th>
                </tr>
              </thead>
              <tbody>
                {Object.entries(tierQuery.data.by_domain_tier)
                  .sort(([a], [b]) => a.localeCompare(b))
                  .map(([domain, tiers]) => (
                    <tr key={domain} className="border-b border-gray-100">
                      <td className="py-1 font-mono">{domain}</td>
                      <td className="py-1 text-right">
                        {tiers["benign"] ?? "·"}
                      </td>
                      <td className="py-1 text-right">
                        {tiers["reviewable"] ?? "·"}
                      </td>
                      <td className="py-1 text-right">
                        {tiers["requires_confirmation"] ?? "·"}
                      </td>
                      <td className="py-1 text-right">
                        {tiers["requires_explicit_authorisation"] ?? "·"}
                      </td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </>
        )}
      </div>
    </div>
  );
}

function ProposalRow({
  proposal,
  isSelected,
  onSelect,
}: {
  proposal: ProposalSummary;
  isSelected: boolean;
  onSelect: () => void;
}) {
  return (
    <li>
      <button
        onClick={onSelect}
        className={`w-full text-left px-2 py-2 rounded text-sm ${
          isSelected ? "bg-blue-100" : "hover:bg-gray-50"
        }`}
      >
        <div className="font-mono text-xs truncate">{proposal.verb_fqn}</div>
        <div className="flex justify-between items-center mt-1">
          <StatusPill status={proposal.status} />
          <span className="text-xs text-gray-500 truncate ml-1">
            {proposal.proposed_by}
          </span>
        </div>
      </button>
    </li>
  );
}

function StatusPill({ status }: { status: string }) {
  const colors: Record<string, string> = {
    DRAFT: "bg-gray-200 text-gray-700",
    STAGED: "bg-yellow-200 text-yellow-800",
    COMMITTED: "bg-green-200 text-green-800",
    ROLLED_BACK: "bg-red-200 text-red-800",
    REJECTED: "bg-red-300 text-red-900",
  };
  return (
    <span
      className={`px-2 py-0.5 rounded text-xs font-semibold ${
        colors[status] ?? "bg-gray-200"
      }`}
    >
      {status}
    </span>
  );
}

function TierBar({
  tier,
  count,
  total,
}: {
  tier: string;
  count: number;
  total: number;
}) {
  const pct = total > 0 ? (count / total) * 100 : 0;
  const tierColors: Record<string, string> = {
    benign: "bg-green-400",
    reviewable: "bg-yellow-400",
    requires_confirmation: "bg-orange-400",
    requires_explicit_authorisation: "bg-red-500",
    "(undeclared)": "bg-gray-300",
  };
  return (
    <div>
      <div className="flex justify-between text-xs mb-0.5">
        <span className="font-mono">{tier}</span>
        <span className="text-gray-600">
          {count} ({pct.toFixed(1)}%)
        </span>
      </div>
      <div className="w-full bg-gray-100 rounded-full h-2">
        <div
          className={`h-2 rounded-full ${
            tierColors[tier] ?? "bg-gray-400"
          }`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}
