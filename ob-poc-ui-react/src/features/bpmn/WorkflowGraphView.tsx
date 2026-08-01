import type { SessionGraph } from "../../api/bpmnTemplates";

/**
 * Camunda-8-style read-only workflow visualiser.
 *
 * Renders the COMPILED workflow graph the designer serves for a session
 * (same DAG -> IR -> plan chain that save/spawn executes), using the
 * server's layered layout: x = execution depth, so flow reads strictly
 * left-to-right in execution order. This is a window onto the compiled
 * artifact, not an editor.
 *
 * Execution overlay: `activeNodeIds` (waiting jobs) render with an
 * amber "token here" ring, like Camunda Operate; when `completed` the
 * whole flow renders green.
 */

const NODE_W = 130;
const NODE_H = 56;
const EVENT_R = 22;
const GATEWAY_R = 24; // half-diagonal of the gateway diamond
const MARGIN_X = 40;
const MARGIN_Y = 40;

interface Props {
  graph: SessionGraph;
  activeNodeIds?: string[];
  completed?: boolean;
}

export function WorkflowGraphView({ graph, activeNodeIds = [], completed = false }: Props) {
  if (!graph.compiles || !graph.graph || !graph.layout) {
    return (
      <div className="text-xs text-red-300 bg-red-950 border border-red-800 rounded p-2 font-mono whitespace-pre-wrap">
        graph does not compile:{"\n"}
        {graph.diagnostics.join("\n")}
      </div>
    );
  }

  const layout = graph.layout;
  const nodes = graph.graph.nodes;
  const edges = graph.graph.edges;

  const maxX = Math.max(...nodes.map((n) => layout[n.id]?.x ?? 0));
  const maxY = Math.max(...nodes.map((n) => layout[n.id]?.y ?? 0));
  const width = maxX + NODE_W + MARGIN_X * 2;
  const height = Math.max(maxY + NODE_H + MARGIN_Y * 2, NODE_H + MARGIN_Y * 2);

  // Node center for edge routing; events are circles, tasks are rects.
  const center = (id: string) => {
    const p = layout[id] ?? { x: 0, y: 0 };
    return { cx: p.x + MARGIN_X + NODE_W / 2, cy: p.y + MARGIN_Y + NODE_H / 2 };
  };
  const nodeById = new Map(nodes.map((n) => [n.id, n]));
  // Horizontal half-extent per shape kind, for edge endpoint routing.
  const shapeHalf = (kind: string) =>
    kind === "start" || kind === "end"
      ? EVENT_R
      : kind === "split" || kind === "join"
        ? GATEWAY_R
        : NODE_W / 2;

  // Edge endpoints: leave from the right edge of the source shape,
  // arrive at the left edge of the target shape (flow order is layout
  // order, so this is always forward).
  const edgePath = (from: string, to: string) => {
    const a = center(from);
    const b = center(to);
    const aHalf = shapeHalf(nodeById.get(from)?.kind ?? "");
    const bHalf = shapeHalf(nodeById.get(to)?.kind ?? "");
    const x1 = a.cx + aHalf;
    const x2 = b.cx - bHalf;
    if (a.cy === b.cy) return `M ${x1} ${a.cy} L ${x2} ${b.cy}`;
    // Orthogonal dog-leg for cross-lane edges, Camunda style.
    const midX = (x1 + x2) / 2;
    return `M ${x1} ${a.cy} L ${midX} ${a.cy} L ${midX} ${b.cy} L ${x2} ${b.cy}`;
  };

  const strokeFor = (id: string) => {
    if (completed) return "#4ade80"; // green-400
    if (activeNodeIds.includes(id)) return "#fbbf24"; // amber-400
    return "#9ca3af"; // gray-400
  };

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className="w-full bg-gray-900 border border-gray-800 rounded"
      style={{ maxHeight: 260 }}
      data-testid="workflow-graph"
    >
      <defs>
        <marker id="wf-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#9ca3af" />
        </marker>
      </defs>

      {edges.map((e) => (
        <path
          key={`${e.from}->${e.to}`}
          d={edgePath(e.from, e.to)}
          fill="none"
          stroke="#9ca3af"
          strokeWidth={1.5}
          markerEnd="url(#wf-arrow)"
        />
      ))}

      {nodes.map((n) => {
        const { cx, cy } = center(n.id);
        const stroke = strokeFor(n.id);
        const active = !completed && activeNodeIds.includes(n.id);
        if (n.kind === "split" || n.kind === "join") {
          // Gateway diamond, Camunda style; glyph from the mode label.
          const glyph = n.label.includes("XOR") ? "×" : n.label.includes("(OR)") ? "○" : "+";
          return (
            <g key={n.id} data-node-id={n.id}>
              <path
                d={`M ${cx} ${cy - GATEWAY_R} L ${cx + GATEWAY_R} ${cy} L ${cx} ${cy + GATEWAY_R} L ${cx - GATEWAY_R} ${cy} Z`}
                fill="#111827"
                stroke={stroke}
                strokeWidth={2}
              />
              <text x={cx} y={cy + 6} textAnchor="middle" fontSize="16" fill="#e5e7eb" fontFamily="monospace">
                {glyph}
              </text>
              <text x={cx} y={cy + GATEWAY_R + 14} textAnchor="middle" fontSize="9" fill="#6b7280" fontFamily="monospace">
                {n.id}
              </text>
            </g>
          );
        }
        if (n.kind === "start" || n.kind === "end") {
          return (
            <g key={n.id} data-node-id={n.id}>
              <circle
                cx={cx}
                cy={cy}
                r={EVENT_R}
                fill="#111827"
                stroke={stroke}
                strokeWidth={n.kind === "end" ? 4 : 2}
              />
              <text x={cx} y={cy + EVENT_R + 14} textAnchor="middle" fontSize="10" fill="#9ca3af" fontFamily="monospace">
                {n.label}
              </text>
            </g>
          );
        }
        return (
          <g key={n.id} data-node-id={n.id}>
            <rect
              x={cx - NODE_W / 2}
              y={cy - NODE_H / 2}
              width={NODE_W}
              height={NODE_H}
              rx={10}
              fill={active ? "#78350f" : "#111827"}
              stroke={stroke}
              strokeWidth={active ? 3 : 2}
            />
            <text x={cx} y={n.plug ? cy - 2 : cy + 4} textAnchor="middle" fontSize="12" fill="#e5e7eb" fontFamily="monospace">
              {n.id.length > 16 ? `${n.id.slice(0, 15)}…` : n.id}
            </text>
            {n.plug && (
              <text x={cx} y={cy + 14} textAnchor="middle" fontSize="9" fill="#6b7280" fontFamily="monospace">
                {n.plug.length > 20 ? `${n.plug.slice(0, 19)}…` : n.plug}
              </text>
            )}
            {active && (
              <circle cx={cx - NODE_W / 2 + 10} cy={cy - NODE_H / 2 + 10} r={5} fill="#fbbf24">
                <animate attributeName="opacity" values="1;0.3;1" dur="1.2s" repeatCount="indefinite" />
              </circle>
            )}
          </g>
        );
      })}
    </svg>
  );
}
