import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  AlertTriangle,
  ArrowLeftFromLine,
  Briefcase,
  Building2,
  ChevronDown,
  ChevronRight,
  Circle,
  Eye,
  FileText,
  GitBranch,
  Layers,
  Loader2,
  Network,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  User,
  Waypoints,
} from "lucide-react";
import {
  constellationApi,
  type ConstellationCaseSummary,
  type ConstellationSummary,
  type HydratedGraphNode,
  type HydratedConstellation,
  type HydratedSlot,
} from "../../../api/constellation";
import type { CbuSummary } from "../../../api/scope";
import { queryKeys } from "../../../lib/query";
import { cn } from "../../../lib/utils";
import type { SessionFeedback } from "../../../api/replV2";

const DEFAULT_MAP_NAME = "struct.lux.ucits.sicav";

function stateBadgeClass(state: string, blocking: boolean): string {
  if (blocking) {
    return "border-[var(--accent-red)]/40 bg-[var(--accent-red)]/10 text-[var(--accent-red)]";
  }

  switch (state) {
    case "approved":
    case "verified":
      return "border-[var(--accent-green)]/40 bg-[var(--accent-green)]/10 text-[var(--accent-green)]";
    case "placeholder":
      return "border-[var(--accent-yellow)]/40 bg-[var(--accent-yellow)]/10 text-[var(--accent-yellow)]";
    case "empty":
      return "border-[var(--border-primary)] bg-[var(--bg-tertiary)] text-[var(--text-muted)]";
    default:
      return "border-[var(--accent-blue)]/40 bg-[var(--accent-blue)]/10 text-[var(--accent-blue)]";
  }
}

function slotIconForType(slotType: string) {
  switch (slotType) {
    case "cbu":
      return Building2;
    case "entity":
      return User;
    case "entity_graph":
      return Network;
    case "case":
      return FileText;
    case "tollgate":
      return ShieldCheck;
    case "mandate":
      return Briefcase;
    default:
      return Circle;
  }
}

function flattenSlots(slots: HydratedSlot[]): HydratedSlot[] {
  return slots.flatMap((slot) => [slot, ...flattenSlots(slot.children)]);
}

function findSlotByPath(
  slots: HydratedSlot[],
  path: string | null,
): HydratedSlot | null {
  if (!path) {
    return null;
  }

  for (const slot of slots) {
    if (slot.path === path) {
      return slot;
    }
    const child = findSlotByPath(slot.children, path);
    if (child) {
      return child;
    }
  }

  return null;
}

function SummaryMetric({
  label,
  value,
  emphasis = false,
}: {
  label: string;
  value: string | number;
  emphasis?: boolean;
}) {
  return (
    <div
      className={cn(
        "rounded-lg border border-[var(--border-primary)] px-3 py-2",
        emphasis && "border-[var(--accent-red)]/30 bg-[var(--accent-red)]/5",
      )}
    >
      <div className="text-[11px] uppercase tracking-[0.14em] text-[var(--text-muted)]">
        {label}
      </div>
      <div className="mt-1 text-lg font-semibold text-[var(--text-primary)]">
        {value}
      </div>
    </div>
  );
}

function ConstellationSummaryStrip({
  summary,
}: {
  summary: ConstellationSummary;
}) {
  return (
    <div className="grid grid-cols-2 gap-2">
      <SummaryMetric label="Progress" value={`${summary.overall_progress}%`} />
      <SummaryMetric label="Complete" value={`${summary.completion_pct}%`} />
      <SummaryMetric
        label="Blocking Slots"
        value={summary.blocking_slots}
        emphasis
      />
      <SummaryMetric label="In Progress" value={summary.slots_in_progress} />
      <SummaryMetric
        label="Mandatory Empty"
        value={summary.slots_empty_mandatory}
      />
      <SummaryMetric label="Placeholders" value={summary.slots_placeholder} />
      {summary.ownership_chain && (
        <>
          <SummaryMetric
            label="Ownership Entities"
            value={summary.ownership_chain.total_entities}
          />
          <SummaryMetric
            label="Ownership Edges"
            value={summary.ownership_chain.total_edges}
          />
        </>
      )}
    </div>
  );
}

function SlotTreeRow({
  slot,
  depth,
  selectedPath,
  onSelect,
}: {
  slot: HydratedSlot;
  depth: number;
  selectedPath: string | null;
  onSelect: (path: string) => void;
}) {
  const [open, setOpen] = useState(true);
  const hasChildren = slot.children.length > 0;
  const selected = selectedPath === slot.path;
  const SlotIcon = slotIconForType(slot.slot_type);

  return (
    <div>
      <div
        className={cn(
          "group flex items-center gap-2 rounded-lg border px-2 py-2 text-left transition-colors",
          selected
            ? "border-[var(--accent-blue)]/40 bg-[var(--accent-blue)]/10"
            : "border-transparent hover:border-[var(--border-primary)] hover:bg-[var(--bg-tertiary)]",
        )}
        style={{ marginLeft: depth * 12 }}
      >
        <button
          type="button"
          onClick={() =>
            hasChildren ? setOpen((current) => !current) : undefined
          }
          className="flex h-5 w-5 items-center justify-center rounded text-[var(--text-muted)] hover:bg-[var(--bg-primary)]"
          aria-label={open ? "Collapse slot" : "Expand slot"}
        >
          {hasChildren ? (
            open ? (
              <ChevronDown size={14} />
            ) : (
              <ChevronRight size={14} />
            )
          ) : (
            <span className="h-2 w-2 rounded-full bg-[var(--border-primary)]" />
          )}
        </button>

        <button
          type="button"
          onClick={() => onSelect(slot.path)}
          className="min-w-0 flex-1 text-left"
        >
          <div className="flex items-center gap-2">
            <SlotIcon
              size={14}
              className="flex-shrink-0 text-[var(--text-muted)]"
            />
            <span className="truncate text-sm font-medium text-[var(--text-primary)]">
              {slot.name.replaceAll("_", " ")}
            </span>
            <span
              className={cn(
                "rounded-full border px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.14em]",
                stateBadgeClass(slot.effective_state, slot.blocking),
              )}
            >
              {slot.effective_state}
            </span>
          </div>
          <div className="mt-1 flex items-center gap-3 text-xs text-[var(--text-muted)]">
            <span>{slot.slot_type}</span>
            <span>{slot.cardinality}</span>
            <span>{slot.progress}%</span>
            {slot.warnings.length > 0 && (
              <span className="flex items-center gap-1 text-[var(--accent-yellow)]">
                <AlertTriangle size={12} />
                {slot.warnings.length}
              </span>
            )}
          </div>
        </button>
      </div>

      {hasChildren && open && (
        <div className="mt-1 space-y-1">
          {slot.children.map((child) => (
            <SlotTreeRow
              key={child.path}
              slot={child}
              depth={depth + 1}
              selectedPath={selectedPath}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function OverlayFieldList({ fields }: { fields: Record<string, unknown> }) {
  const entries = Object.entries(fields);

  if (entries.length === 0) {
    return <div className="text-xs text-[var(--text-muted)]">No fields.</div>;
  }

  return (
    <div className="space-y-1">
      {entries.map(([key, value]) => (
        <div
          key={key}
          className="grid grid-cols-[minmax(0,10rem)_1fr] gap-2 rounded-md bg-[var(--bg-primary)] px-2 py-1.5 text-xs"
        >
          <div className="truncate font-medium text-[var(--text-secondary)]">
            {key}
          </div>
          <div className="overflow-x-auto text-[var(--text-muted)]">
            {typeof value === "string"
              ? value
              : value === null
                ? "null"
                : JSON.stringify(value)}
          </div>
        </div>
      ))}
    </div>
  );
}

function formatCaseLabel(item: ConstellationCaseSummary): string {
  const parts = [
    item.case_type || "Case",
    item.status || "unknown",
    item.opened_at ? new Date(item.opened_at).toLocaleDateString() : null,
  ].filter(Boolean);
  return parts.join(" • ");
}

function findGraphNode(
  nodes: HydratedGraphNode[],
  entityId: string,
): HydratedGraphNode | undefined {
  return nodes.find((node) => node.entity_id === entityId);
}

function OwnershipGraphCanvas({
  nodes,
  edges,
}: {
  nodes: HydratedGraphNode[];
  edges: HydratedSlot["graph_edges"];
}) {
  if (nodes.length === 0) {
    return (
      <div className="rounded-md border border-dashed border-[var(--border-primary)] px-3 py-6 text-center text-sm text-[var(--text-muted)]">
        No ownership graph nodes available.
      </div>
    );
  }

  const targetDepths = new Map<string, number>();
  const targetIds = new Set(edges.map((edge) => edge.to_entity_id));

  for (const edge of edges) {
    const current = targetDepths.get(edge.to_entity_id);
    const nextDepth = Math.max(edge.depth, 1);
    if (current == null || nextDepth < current) {
      targetDepths.set(edge.to_entity_id, nextDepth);
    }
  }

  const grouped = new Map<number, HydratedGraphNode[]>();
  for (const node of nodes) {
    const depth =
      targetDepths.get(node.entity_id) ??
      (targetIds.has(node.entity_id) ? 1 : 0);
    const bucket = grouped.get(depth) ?? [];
    bucket.push(node);
    grouped.set(depth, bucket);
  }

  for (const bucket of grouped.values()) {
    bucket.sort((lhs, rhs) =>
      (lhs.name ?? lhs.entity_id).localeCompare(rhs.name ?? rhs.entity_id),
    );
  }

  const columns = Array.from(grouped.keys()).sort((lhs, rhs) => lhs - rhs);
  const columnWidth = 220;
  const rowHeight = 92;
  const paddingX = 40;
  const paddingY = 36;
  const nodeWidth = 150;
  const nodeHeight = 50;
  const canvasWidth = Math.max(
    420,
    columns.length * columnWidth + paddingX * 2,
  );
  const maxRows = Math.max(
    ...Array.from(grouped.values()).map((bucket) => bucket.length),
    1,
  );
  const canvasHeight = Math.max(220, maxRows * rowHeight + paddingY * 2);
  const positions = new Map<string, { x: number; y: number }>();

  for (const column of columns) {
    const bucket = grouped.get(column) ?? [];
    bucket.forEach((node, index) => {
      positions.set(node.entity_id, {
        x: paddingX + columns.indexOf(column) * columnWidth + 12,
        y: paddingY + index * rowHeight,
      });
    });
  }

  return (
    <div className="overflow-x-auto rounded-md border border-[var(--border-primary)] bg-[var(--bg-secondary)] p-2">
      <svg width={canvasWidth} height={canvasHeight} className="block">
        {edges.map((edge, index) => {
          const from = positions.get(edge.from_entity_id);
          const to = positions.get(edge.to_entity_id);
          if (!from || !to) {
            return null;
          }
          return (
            <g key={`${edge.from_entity_id}-${edge.to_entity_id}-${index}`}>
              <line
                x1={from.x + nodeWidth}
                y1={from.y + nodeHeight / 2}
                x2={to.x}
                y2={to.y + nodeHeight / 2}
                stroke="var(--border-primary)"
                strokeWidth="2"
              />
              <text
                x={(from.x + nodeWidth + to.x) / 2}
                y={(from.y + to.y) / 2 + nodeHeight / 2 - 6}
                fill="var(--text-muted)"
                fontSize="10"
                textAnchor="middle"
              >
                {edge.percentage != null
                  ? `${edge.percentage}%`
                  : (edge.ownership_type ?? "")}
              </text>
            </g>
          );
        })}
        {nodes.map((node) => {
          const position = positions.get(node.entity_id);
          if (!position) {
            return null;
          }
          return (
            <g key={node.entity_id}>
              <rect
                x={position.x}
                y={position.y}
                rx="10"
                ry="10"
                width={nodeWidth}
                height={nodeHeight}
                fill="var(--bg-primary)"
                stroke="var(--border-primary)"
              />
              <text
                x={position.x + 12}
                y={position.y + 20}
                fill="var(--text-primary)"
                fontSize="12"
                fontWeight="600"
              >
                {(node.name ?? node.entity_id).slice(0, 24)}
              </text>
              <text
                x={position.x + 12}
                y={position.y + 36}
                fill="var(--text-muted)"
                fontSize="10"
              >
                {(node.entity_type ?? "unknown").slice(0, 24)}
              </text>
            </g>
          );
        })}
        {columns.map((column) => (
          <text
            key={column}
            x={paddingX + columns.indexOf(column) * columnWidth + nodeWidth / 2}
            y={18}
            fill="var(--text-muted)"
            fontSize="10"
            textAnchor="middle"
          >
            Depth {column}
          </text>
        ))}
      </svg>
    </div>
  );
}

function SlotInspector({
  slot,
  hydrated,
  cbuName,
  onPromptAgent,
}: {
  slot: HydratedSlot | null;
  hydrated: HydratedConstellation;
  cbuName: string;
  onPromptAgent?: (prompt: string) => void;
}) {
  if (!slot) {
    return (
      <div className="rounded-xl border border-dashed border-[var(--border-primary)] px-4 py-5 text-sm text-[var(--text-muted)]">
        Select a slot to inspect its reducer state, warnings, overlays, and
        action surface.
      </div>
    );
  }

  return (
    <div className="space-y-3 rounded-xl border border-[var(--border-primary)] bg-[var(--bg-primary)] p-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
            Slot Inspector
          </div>
          <div className="mt-1 text-lg font-semibold text-[var(--text-primary)]">
            {slot.name.replaceAll("_", " ")}
          </div>
          <div className="mt-1 text-xs text-[var(--text-muted)]">
            {slot.path}
          </div>
        </div>
        <span
          className={cn(
            "rounded-full border px-2 py-1 text-[10px] font-semibold uppercase tracking-[0.14em]",
            stateBadgeClass(slot.effective_state, slot.blocking),
          )}
        >
          {slot.effective_state}
        </span>
      </div>

      <div className="grid grid-cols-2 gap-2 text-sm">
        <div className="rounded-lg border border-[var(--border-primary)] px-3 py-2">
          <div className="text-[11px] uppercase tracking-[0.14em] text-[var(--text-muted)]">
            Computed
          </div>
          <div className="mt-1 font-medium text-[var(--text-primary)]">
            {slot.computed_state}
          </div>
        </div>
        <div className="rounded-lg border border-[var(--border-primary)] px-3 py-2">
          <div className="text-[11px] uppercase tracking-[0.14em] text-[var(--text-muted)]">
            Progress
          </div>
          <div className="mt-1 font-medium text-[var(--text-primary)]">
            {slot.progress}%
          </div>
        </div>
        <div className="rounded-lg border border-[var(--border-primary)] px-3 py-2">
          <div className="text-[11px] uppercase tracking-[0.14em] text-[var(--text-muted)]">
            Slot Type
          </div>
          <div className="mt-1 font-medium text-[var(--text-primary)]">
            {slot.slot_type}
          </div>
        </div>
        <div className="rounded-lg border border-[var(--border-primary)] px-3 py-2">
          <div className="text-[11px] uppercase tracking-[0.14em] text-[var(--text-muted)]">
            Cardinality
          </div>
          <div className="mt-1 font-medium text-[var(--text-primary)]">
            {slot.cardinality}
          </div>
        </div>
      </div>

      <div className="rounded-lg border border-[var(--border-primary)] px-3 py-3">
        <div className="flex items-center justify-between gap-3">
          <div className="text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
            Context
          </div>
          <div className="text-[11px] text-[var(--text-muted)]">
            map {hydrated.map_revision}
          </div>
        </div>
        <div className="mt-2 space-y-1 text-sm text-[var(--text-secondary)]">
          <div>CBU: {hydrated.cbu_id}</div>
          <div>Case: {hydrated.case_id ?? "No case bound"}</div>
          <div>Entity: {slot.entity_id ?? "None"}</div>
          <div>Record: {slot.record_id ?? "None"}</div>
        </div>
      </div>

      {onPromptAgent && (
        <div className="grid gap-2 md:grid-cols-2">
          <button
            type="button"
            onClick={() =>
              onPromptAgent(
                `Explain the current constellation state for slot '${slot.path}' in CBU '${cbuName}'. Include effective state, computed state, warnings, and the most important next step.`,
              )
            }
            className="rounded-lg border border-[var(--border-primary)] bg-[var(--bg-secondary)] px-3 py-2 text-sm font-medium text-[var(--text-primary)] hover:bg-[var(--bg-tertiary)]"
          >
            Ask Agent About This Slot
          </button>
          <button
            type="button"
            onClick={() =>
              onPromptAgent(
                `Why is slot '${slot.path}' blocked for CBU '${cbuName}'? Use the current constellation blocked verbs, warnings, and effective state to explain the blocker and recommend the next best action.`,
              )
            }
            className="rounded-lg bg-[var(--accent-blue)] px-3 py-2 text-sm font-medium text-white"
          >
            Ask Why Blocked
          </button>
        </div>
      )}

      {slot.slot_type === "entity_graph" && (
        <div className="rounded-lg border border-[var(--border-primary)] px-3 py-3">
          <div className="flex items-center gap-2 text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
            <Network size={12} />
            Ownership Chain
          </div>
          <div className="mt-2 grid grid-cols-2 gap-2 text-sm">
            <div className="rounded-md bg-[var(--bg-secondary)] px-3 py-2">
              Nodes: {slot.graph_node_count ?? 0}
            </div>
            <div className="rounded-md bg-[var(--bg-secondary)] px-3 py-2">
              Edges: {slot.graph_edge_count ?? 0}
            </div>
          </div>
          <div className="mt-3">
            <OwnershipGraphCanvas
              nodes={slot.graph_nodes}
              edges={slot.graph_edges}
            />
          </div>
          <div className="mt-3 space-y-2">
            {slot.graph_nodes.length > 0 ? (
              <div className="space-y-2">
                <div className="text-[11px] uppercase tracking-[0.14em] text-[var(--text-muted)]">
                  Nodes
                </div>
                {slot.graph_nodes.map((node) => (
                  <div
                    key={node.entity_id}
                    className="rounded-md bg-[var(--bg-secondary)] px-3 py-2 text-sm"
                  >
                    <div className="font-medium text-[var(--text-primary)]">
                      {node.name ?? node.entity_id}
                    </div>
                    <div className="mt-1 text-xs text-[var(--text-muted)]">
                      {node.entity_type ?? "unknown type"} • {node.entity_id}
                    </div>
                  </div>
                ))}
              </div>
            ) : null}
            {slot.graph_edges.length > 0 ? (
              <div className="space-y-2">
                <div className="text-[11px] uppercase tracking-[0.14em] text-[var(--text-muted)]">
                  Edges
                </div>
                {slot.graph_edges.map((edge, index) => {
                  const fromNode = findGraphNode(
                    slot.graph_nodes,
                    edge.from_entity_id,
                  );
                  const toNode = findGraphNode(
                    slot.graph_nodes,
                    edge.to_entity_id,
                  );
                  return (
                    <div
                      key={`${edge.from_entity_id}-${edge.to_entity_id}-${index}`}
                      className="rounded-md bg-[var(--bg-secondary)] px-3 py-2 text-sm"
                    >
                      <div className="font-medium text-[var(--text-primary)]">
                        {(fromNode?.name ?? edge.from_entity_id).slice(0, 48)} →{" "}
                        {(toNode?.name ?? edge.to_entity_id).slice(0, 48)}
                      </div>
                      <div className="mt-1 text-xs text-[var(--text-muted)]">
                        depth {edge.depth}
                        {edge.ownership_type ? ` • ${edge.ownership_type}` : ""}
                        {edge.percentage != null
                          ? ` • ${edge.percentage}%`
                          : ""}
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : null}
          </div>
        </div>
      )}

      {slot.warnings.length > 0 && (
        <div className="rounded-lg border border-[var(--accent-yellow)]/30 bg-[var(--accent-yellow)]/10 px-3 py-3">
          <div className="flex items-center gap-2 text-xs uppercase tracking-[0.14em] text-[var(--accent-yellow)]">
            <AlertTriangle size={12} />
            Warnings
          </div>
          <ul className="mt-2 space-y-2 text-sm text-[var(--text-primary)]">
            {slot.warnings.map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        </div>
      )}

      <div className="grid gap-3 md:grid-cols-2">
        <div className="rounded-lg border border-[var(--border-primary)] px-3 py-3">
          <div className="flex items-center gap-2 text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
            <Waypoints size={12} />
            Available Verbs
          </div>
          <div className="mt-2 flex flex-wrap gap-2">
            {slot.available_verbs.length > 0 ? (
              slot.available_verbs.map((verb) => (
                <span
                  key={verb}
                  className="rounded-full bg-[var(--bg-secondary)] px-2 py-1 text-xs text-[var(--text-primary)]"
                >
                  {verb}
                </span>
              ))
            ) : (
              <span className="text-sm text-[var(--text-muted)]">
                No available verbs.
              </span>
            )}
          </div>
        </div>

        <div className="rounded-lg border border-[var(--border-primary)] px-3 py-3">
          <div className="flex items-center gap-2 text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
            <ShieldAlert size={12} />
            Blocked Verbs
          </div>
          <div className="mt-2 space-y-2">
            {slot.blocked_verbs.length > 0 ? (
              slot.blocked_verbs.map((blocked) => (
                <div
                  key={blocked.verb}
                  className="rounded-md bg-[var(--bg-secondary)] px-2 py-2"
                >
                  <div className="text-sm font-medium text-[var(--text-primary)]">
                    {blocked.verb}
                  </div>
                  <ul className="mt-1 space-y-1 text-xs text-[var(--text-muted)]">
                    {blocked.reasons.map((reason) => (
                      <li key={`${blocked.verb}-${reason.message}`}>
                        {reason.message}
                      </li>
                    ))}
                  </ul>
                </div>
              ))
            ) : (
              <span className="text-sm text-[var(--text-muted)]">
                No blocked verbs.
              </span>
            )}
          </div>
        </div>
      </div>

      <div className="rounded-lg border border-[var(--border-primary)] px-3 py-3">
        <div className="flex items-center gap-2 text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
          <GitBranch size={12} />
          Overlays
        </div>
        <div className="mt-2 space-y-2">
          {slot.overlays.length > 0 ? (
            slot.overlays.map((overlay, index) => (
              <details
                key={`${overlay.source_name}-${index}`}
                className="rounded-md bg-[var(--bg-secondary)] px-2 py-2"
              >
                <summary className="cursor-pointer list-none text-sm font-medium text-[var(--text-primary)]">
                  {overlay.source_name}
                </summary>
                <div className="mt-2">
                  <OverlayFieldList fields={overlay.fields} />
                </div>
              </details>
            ))
          ) : (
            <div className="text-sm text-[var(--text-muted)]">
              No overlays bound to this slot.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export function ConstellationPanel({
  selectedCbu,
  sessionFeedback,
  className,
  onPromptAgent,
}: {
  selectedCbu: CbuSummary | null;
  sessionFeedback?: SessionFeedback;
  className?: string;
  onPromptAgent?: (prompt: string) => void;
}) {
  const [caseIdDraft, setCaseIdDraft] = useState("");
  const [caseIdApplied, setCaseIdApplied] = useState<string | undefined>(
    undefined,
  );
  const [selectedSlotPath, setSelectedSlotPath] = useState<string | null>(null);

  const sessionHydrated =
    (sessionFeedback?.tos?.hydrated_constellation as HydratedConstellation | undefined) ??
    undefined;
  const sessionSummary = sessionFeedback?.tos?.progress_summary;
  const sessionAllSlots = useMemo(
    () => flattenSlots(sessionHydrated?.slots ?? []),
    [sessionHydrated],
  );
  const sessionSelectedSlot = useMemo(() => {
    const fromPath = findSlotByPath(sessionHydrated?.slots ?? [], selectedSlotPath);
    return fromPath ?? sessionAllSlots[0] ?? null;
  }, [selectedSlotPath, sessionAllSlots, sessionHydrated]);

  const constellationQuery = useQuery({
    queryKey: queryKeys.constellation.detail(
      selectedCbu?.id ?? "",
      caseIdApplied,
      DEFAULT_MAP_NAME,
    ),
    queryFn: () =>
      constellationApi.getConstellation(selectedCbu!.id, {
        caseId: caseIdApplied,
        mapName: DEFAULT_MAP_NAME,
      }),
    enabled: !!selectedCbu?.id,
    staleTime: 2000,
  });

  const summaryQuery = useQuery({
    queryKey: queryKeys.constellation.summary(
      selectedCbu?.id ?? "",
      caseIdApplied,
      DEFAULT_MAP_NAME,
    ),
    queryFn: () =>
      constellationApi.getSummary(selectedCbu!.id, {
        caseId: caseIdApplied,
        mapName: DEFAULT_MAP_NAME,
      }),
    enabled: !!selectedCbu?.id,
    staleTime: 2000,
  });

  const casesQuery = useQuery({
    queryKey: queryKeys.constellation.cases(selectedCbu?.id ?? ""),
    queryFn: () => constellationApi.getCases(selectedCbu!.id),
    enabled: !!selectedCbu?.id,
    staleTime: 30_000,
  });

  useEffect(() => {
    const cases = casesQuery.data ?? [];

    if (cases.length === 0) {
      setCaseIdDraft("");
      setCaseIdApplied(undefined);
      return;
    }

    const stillValid = caseIdApplied
      ? cases.some((item) => item.case_id === caseIdApplied)
      : false;

    if (stillValid) {
      return;
    }

    setCaseIdDraft(cases[0].case_id);
    setCaseIdApplied(cases[0].case_id);
  }, [caseIdApplied, casesQuery.data, selectedCbu?.id]);

  const allSlots = useMemo(
    () => flattenSlots(constellationQuery.data?.slots ?? []),
    [constellationQuery.data],
  );

  const selectedSlot = useMemo(() => {
    const fromPath = findSlotByPath(
      constellationQuery.data?.slots ?? [],
      selectedSlotPath,
    );
    return fromPath ?? allSlots[0] ?? null;
  }, [allSlots, constellationQuery.data, selectedSlotPath]);
  const selectedCase = useMemo(
    () =>
      (casesQuery.data ?? []).find((item) => item.case_id === caseIdApplied) ??
      null,
    [caseIdApplied, casesQuery.data],
  );

  const refreshAll = async () => {
    await Promise.all([constellationQuery.refetch(), summaryQuery.refetch()]);
  };

  if (sessionHydrated) {
    return (
      <div
        className={cn(
          "flex min-h-0 flex-1 flex-col border-t border-[var(--border-primary)]",
          className,
        )}
      >
        <div className="border-b border-[var(--border-primary)] px-4 py-3">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <div className="text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
                Session Context
              </div>
              <div className="mt-1 truncate text-base font-semibold text-[var(--text-primary)]">
                {sessionFeedback?.tos.workspace.replaceAll("_", " ")}
              </div>
              <div className="mt-1 text-xs text-[var(--text-muted)]">
                {sessionFeedback?.tos.constellation_family} • {sessionFeedback?.tos.constellation_map}
              </div>
              {sessionFeedback?.stale_warning && (
                <div className="mt-2 rounded-lg border border-[var(--accent-yellow)]/30 bg-[var(--accent-yellow)]/10 px-3 py-2 text-xs text-[var(--accent-yellow)]">
                  Restored frame may be stale. Re-hydrate before relying on pronouns for writes.
                </div>
              )}
              {/* Stack navigation indicator */}
              {sessionFeedback && (sessionFeedback.stack_depth > 1 || sessionFeedback.tos_is_peek) && (
                <div className="mt-2 flex items-center gap-2 text-xs text-[var(--text-muted)]">
                  <Layers size={12} />
                  <span>Stack depth: {sessionFeedback.stack_depth}</span>
                  {sessionFeedback.tos_is_peek && (
                    <span className="inline-flex items-center gap-0.5 rounded bg-indigo-50 px-1.5 py-0.5 text-[10px] font-medium text-indigo-700 border border-indigo-200">
                      <Eye size={10} /> PEEK
                    </span>
                  )}
                  {sessionFeedback.stack_depth > 1 && (
                    <button
                      onClick={() => onPromptAgent?.("session.pop")}
                      className="ml-auto flex items-center gap-1 rounded border border-[var(--border-secondary)] px-1.5 py-0.5 text-[10px] hover:bg-[var(--bg-hover)]"
                    >
                      <ArrowLeftFromLine size={10} />
                      Pop
                    </button>
                  )}
                </div>
              )}
            </div>
          </div>
          {sessionSummary && (
            <div className="mt-3 grid grid-cols-3 gap-2">
              <SummaryMetric label="Slots" value={sessionSummary.total_slots} />
              <SummaryMetric label="Complete" value={`${sessionSummary.completion_pct}%`} />
              <SummaryMetric label="Blocking" value={sessionSummary.blocking_slots} emphasis={sessionSummary.blocking_slots > 0} />
            </div>
          )}
        </div>
        <div className="flex-1 overflow-auto px-4 py-4">
          <div className="space-y-4">
            <div className="rounded-xl border border-[var(--border-primary)] bg-[var(--bg-primary)]">
              <div className="border-b border-[var(--border-primary)] px-4 py-3">
                <div className="text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
                  Slot Tree
                </div>
                <div className="mt-1 text-sm text-[var(--text-secondary)]">
                  Session-scoped hydrated top-of-stack constellation.
                </div>
              </div>
              <div className="space-y-1 px-2 py-3">
                {sessionHydrated.slots.map((slot) => (
                  <SlotTreeRow
                    key={slot.path}
                    slot={slot}
                    depth={0}
                    selectedPath={sessionSelectedSlot?.path ?? null}
                    onSelect={setSelectedSlotPath}
                  />
                ))}
              </div>
            </div>

            <SlotInspector
              slot={sessionSelectedSlot}
              hydrated={sessionHydrated}
              cbuName={selectedCbu?.name ?? sessionFeedback?.tos.workspace ?? "session context"}
              onPromptAgent={onPromptAgent}
            />
          </div>
        </div>
      </div>
    );
  }

  if (!selectedCbu) {
    return (
      <div className={cn("border-t border-[var(--border-primary)]", className)}>
        <div className="px-4 py-4">
          <div className="text-sm font-medium text-[var(--text-primary)]">
            Constellation
          </div>
          <div className="mt-2 rounded-xl border border-dashed border-[var(--border-primary)] px-4 py-5 text-sm text-[var(--text-muted)]">
            Load or select a CBU from session scope to inspect the operating
            constellation. Group clearance and linked delta-KYC context can be
            layered onto this view when present.
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      className={cn(
        "flex min-h-0 flex-1 flex-col border-t border-[var(--border-primary)]",
        className,
      )}
    >
      <div className="border-b border-[var(--border-primary)] px-4 py-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
              Constellation
            </div>
            <div className="mt-1 truncate text-base font-semibold text-[var(--text-primary)]">
              {selectedCbu.name}
            </div>
            <div className="mt-1 text-xs text-[var(--text-muted)]">
              {DEFAULT_MAP_NAME}
              {caseIdApplied
                ? ` • linked clearance case ${caseIdApplied}`
                : " • structure-only operating view"}
            </div>
            {constellationQuery.dataUpdatedAt > 0 && (
              <div className="mt-1 text-[10px] uppercase tracking-[0.12em] text-[var(--text-muted)]">
                Last refreshed{" "}
                {new Date(
                  constellationQuery.dataUpdatedAt,
                ).toLocaleTimeString()}
              </div>
            )}
          </div>
          <button
            type="button"
            onClick={() => {
              void refreshAll();
            }}
            className="rounded-lg border border-[var(--border-primary)] p-2 text-[var(--text-muted)] hover:bg-[var(--bg-tertiary)] hover:text-[var(--text-primary)]"
            title="Refresh constellation"
          >
            {constellationQuery.isFetching || summaryQuery.isFetching ? (
              <Loader2 size={14} className="animate-spin" />
            ) : (
              <RefreshCw size={14} />
            )}
          </button>
        </div>

        <div className="mt-3 flex gap-2">
          <select
            value={caseIdDraft}
            onChange={(event) => {
              const nextValue = event.target.value;
              setCaseIdDraft(nextValue);
              setCaseIdApplied(nextValue || undefined);
            }}
            className="min-w-0 flex-1 rounded-lg border border-[var(--border-primary)] bg-[var(--bg-primary)] px-3 py-2 text-sm text-[var(--text-primary)] outline-none ring-0"
          >
            <option value="">No linked case selected</option>
            {(casesQuery.data ?? []).map((item) => (
              <option key={item.case_id} value={item.case_id}>
                {formatCaseLabel(item)}
              </option>
            ))}
          </select>
          <button
            type="button"
            onClick={() => {
              setCaseIdDraft("");
              setCaseIdApplied(undefined);
            }}
            className="rounded-lg border border-[var(--border-primary)] px-3 py-2 text-sm text-[var(--text-secondary)]"
          >
            Clear
          </button>
        </div>
        {selectedCase && (
          <div className="mt-3 rounded-lg border border-[var(--border-primary)] bg-[var(--bg-primary)] px-3 py-3 text-sm">
            <div className="text-[11px] uppercase tracking-[0.14em] text-[var(--text-muted)]">
              Linked Case
            </div>
            <div className="mt-2 grid grid-cols-2 gap-2">
              <div>
                <div className="text-[11px] uppercase tracking-[0.12em] text-[var(--text-muted)]">
                  Type
                </div>
                <div className="mt-1 text-[var(--text-primary)]">
                  {selectedCase.case_type ?? "Unknown"}
                </div>
              </div>
              <div>
                <div className="text-[11px] uppercase tracking-[0.12em] text-[var(--text-muted)]">
                  Status
                </div>
                <div className="mt-1 text-[var(--text-primary)]">
                  {selectedCase.status ?? "Unknown"}
                </div>
              </div>
              <div className="col-span-2">
                <div className="text-[11px] uppercase tracking-[0.12em] text-[var(--text-muted)]">
                  Opened
                </div>
                <div className="mt-1 text-[var(--text-primary)]">
                  {selectedCase.opened_at
                    ? new Date(selectedCase.opened_at).toLocaleString()
                    : "Unknown"}
                </div>
              </div>
            </div>
          </div>
        )}
        {casesQuery.data && casesQuery.data.length === 0 && (
          <div className="mt-2 text-xs text-[var(--text-muted)]">
            No linked clearance or delta-KYC case found for this CBU.
            Constellation is showing structure-only operating state.
          </div>
        )}
      </div>

      <div className="flex-1 overflow-auto px-4 py-4">
        {constellationQuery.isLoading || summaryQuery.isLoading ? (
          <div className="flex items-center justify-center py-12">
            <Loader2
              size={20}
              className="animate-spin text-[var(--text-muted)]"
            />
          </div>
        ) : constellationQuery.error || summaryQuery.error ? (
          <div className="rounded-xl border border-[var(--accent-red)]/30 bg-[var(--accent-red)]/10 px-4 py-4 text-sm text-[var(--accent-red)]">
            {constellationQuery.error instanceof Error
              ? constellationQuery.error.message
              : summaryQuery.error instanceof Error
                ? summaryQuery.error.message
                : "Failed to load constellation"}
          </div>
        ) : constellationQuery.data && summaryQuery.data ? (
          <div className="space-y-4">
            <ConstellationSummaryStrip summary={summaryQuery.data} />

            <div className="rounded-xl border border-[var(--border-primary)] bg-[var(--bg-primary)]">
              <div className="border-b border-[var(--border-primary)] px-4 py-3">
                <div className="text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">
                  Slot Tree
                </div>
                <div className="mt-1 text-sm text-[var(--text-secondary)]">
                  Effective state drives visual status. Inspector includes
                  computed state and override-aware action surface.
                </div>
              </div>
              <div className="space-y-1 px-2 py-3">
                {constellationQuery.data.slots.map((slot) => (
                  <SlotTreeRow
                    key={slot.path}
                    slot={slot}
                    depth={0}
                    selectedPath={selectedSlot?.path ?? null}
                    onSelect={setSelectedSlotPath}
                  />
                ))}
              </div>
            </div>

            <SlotInspector
              slot={selectedSlot}
              hydrated={constellationQuery.data}
              cbuName={selectedCbu.name}
              onPromptAgent={onPromptAgent}
            />
          </div>
        ) : (
          <div className="rounded-xl border border-dashed border-[var(--border-primary)] px-4 py-5 text-sm text-[var(--text-muted)]">
            No constellation data available for this CBU.
          </div>
        )}
      </div>
    </div>
  );
}

export default ConstellationPanel;
