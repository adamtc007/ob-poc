import { useEffect, useRef, useState } from "react";
import { bpmnApi } from "@/api/bpmn";
import type { NodeInfo, WorkflowInstanceDetail } from "@/api/bpmn";

interface Props {
  instanceId: string;
  onRefresh: () => void;
}

function domainColor(domain: string | null): string {
  if (!domain) return "";
  if (domain === "ob-poc") return "text-violet-300";
  if (domain === "dmn-lite") return "text-cyan-300";
  return "text-amber-300";
}

function NodeRow({
  node,
  isCurrent,
  isVisited,
  isWaiting,
}: {
  node: NodeInfo;
  isCurrent: boolean;
  isVisited: boolean;
  isWaiting: boolean;
}) {
  const isCrossDomain = node.target_domain !== null;

  if (node.kind === "gateway") {
    return (
      <div className="flex gap-1 ml-4 text-xs text-gray-500 font-mono">
        <span>◇</span>
        <span>{node.label}</span>
      </div>
    );
  }

  return (
    <div
      className={`flex items-center gap-2 text-xs font-mono px-2 py-1 rounded transition-colors ${
        isCurrent
          ? "bg-blue-800 text-blue-100 border border-blue-500"
          : isVisited
          ? "text-green-400"
          : "text-gray-500"
      }`}
    >
      <span>{isVisited && !isCurrent ? "✓" : isCurrent ? "▶" : "○"}</span>
      <span className={isCrossDomain && !isCurrent ? domainColor(node.target_domain) : undefined}>
        {node.label}
      </span>
      {node.fqn && (
        <span className={`ml-1 text-xs opacity-60 ${domainColor(node.target_domain)}`}>
          [{node.fqn}]
        </span>
      )}
      {isCurrent && isWaiting && (
        <span className="ml-auto text-yellow-400 animate-pulse">waiting…</span>
      )}
    </div>
  );
}

export function WorkflowPanel({ instanceId, onRefresh }: Props) {
  const [detail, setDetail] = useState<WorkflowInstanceDetail | null>(null);
  const [stepping, setStepping] = useState(false);
  const esRef = useRef<EventSource | null>(null);

  useEffect(() => {
    bpmnApi.getInstance(instanceId).then(setDetail).catch(console.error);

    esRef.current = bpmnApi.subscribeToEvents(instanceId, () => {
      bpmnApi.getInstance(instanceId).then(setDetail).catch(console.error);
      onRefresh();
    });

    return () => {
      esRef.current?.close();
    };
  }, [instanceId, onRefresh]);

  const handleNextStep = async () => {
    setStepping(true);
    try {
      await bpmnApi.nextStep(instanceId);
      const updated = await bpmnApi.getInstance(instanceId);
      setDetail(updated);
      onRefresh();
    } catch (e) {
      console.error(e);
    } finally {
      setStepping(false);
    }
  };

  if (!detail) return <div className="p-4 text-sm text-gray-400">Loading…</div>;

  const isWaiting =
    detail.status.includes("WaitingOnInvocation") ||
    detail.status.includes("WaitingOnSubmission");
  const isComplete = detail.status.includes("Completed");
  const isFailed = detail.status.includes("Failed");

  const nodes = detail.nodes ?? [];
  const nodeIds = nodes.map((n) => n.id);
  const currentIdx = nodeIds.indexOf(detail.current_node);
  const currentNodeInfo = nodes.find((n) => n.id === detail.current_node);

  return (
    <div className="p-4 space-y-4">
      {/* Status bar */}
      <div className="flex items-center gap-3">
        <span
          className={`text-xs font-mono px-2 py-1 rounded ${
            isComplete
              ? "bg-green-900 text-green-300"
              : isFailed
              ? "bg-red-900 text-red-300"
              : isWaiting
              ? "bg-yellow-900 text-yellow-300"
              : "bg-blue-900 text-blue-300"
          }`}
        >
          {detail.status}
        </span>
        <span className="text-xs text-gray-400">CBU type: {detail.cbu_type}</span>
      </div>

      {/* Domain legend */}
      <div className="flex gap-3 text-xs font-mono opacity-70">
        <span className="text-violet-300">■ ob-poc</span>
        <span className="text-cyan-300">■ dmn-lite</span>
      </div>

      {/* Node pipeline — server-driven with fqn/target_domain */}
      <div className="space-y-1">
        {nodes.map((node, idx) => {
          const isCurrent = node.id === detail.current_node;
          const isVisited = idx < currentIdx || isComplete;
          return (
            <NodeRow
              key={node.id}
              node={node}
              isCurrent={isCurrent}
              isVisited={isVisited}
              isWaiting={isCurrent && isWaiting}
            />
          );
        })}
      </div>

      {/* Variables */}
      {Object.keys(detail.variables).length > 0 && (
        <div className="text-xs">
          <div className="text-gray-400 mb-1">Variables</div>
          <pre className="bg-gray-900 p-2 rounded text-green-300 overflow-x-auto">
            {JSON.stringify(detail.variables, null, 2)}
          </pre>
        </div>
      )}

      {/* Next step */}
      {(isWaiting || (!isComplete && !isFailed)) && !isComplete && (
        <button
          onClick={handleNextStep}
          disabled={stepping}
          className="w-full text-xs bg-blue-700 hover:bg-blue-600 disabled:bg-gray-700 text-white px-3 py-2 rounded font-mono transition-colors"
        >
          {stepping
            ? "Stepping…"
            : `▶ Next: ${currentNodeInfo?.label ?? detail.current_node}`}
        </button>
      )}

      {isComplete && (
        <div className="text-xs text-green-400 font-mono text-center">
          ✓ CBU Operational
        </div>
      )}
    </div>
  );
}
