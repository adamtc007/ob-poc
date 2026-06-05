import { useEffect, useState, useMemo } from "react";
import { bpmnApi } from "@/api/bpmn";
import type { VisualGraphDto, WorkflowInstanceDetail } from "@/api/bpmn";
import { ConstellationCanvas } from "../observatory/components/ConstellationCanvas";
import { DmnTableViewer } from "./DmnTableViewer";
import type { GraphSceneModel, SceneNode, SceneEdge } from "@/types/observatory";

interface Props {
  instanceId: string;
  onRefresh: () => void;
}

export function projectVisualGraph(
  visualGraph: VisualGraphDto,
  currentNodeId: string,
  currentStatus: string,
  visitedNodeIds: Set<string>
): GraphSceneModel {
  return {
    generation: Date.now(),
    level: "core",
    layout_strategy: "tree_dag",
    nodes: visualGraph.nodes.map((n): SceneNode => {
      let nodeState: string | undefined = undefined;

      if (n.id === currentNodeId) {
        nodeState = currentStatus.includes("Waiting") ? "blocked" : "filled";
      } else if (visitedNodeIds.has(n.id)) {
        nodeState = "complete";
      } else {
        nodeState = "empty";
      }

      let nodeType: SceneNode["node_type"] = "entity";
      if (n.kind === "start") nodeType = "case";
      if (n.kind === "end") nodeType = "tollgate";
      if (n.kind === "split" || n.kind === "join") nodeType = "cbu";

      return {
        id: n.id,
        label: n.label,
        node_type: nodeType,
        state: nodeState,
        progress: nodeState === "complete" ? 100 : 0,
        blocking: nodeState === "blocked",
        depth: 0,
        child_count: 0,
        badges: n.plug ? [{ badge_type: "info", label: n.plug }] : []
      };
    }),
    edges: visualGraph.edges.map((e): SceneEdge => ({
      source: e.from,
      target: e.to,
      edge_type: "control",
      weight: 1.0,
      label: e.condition ?? undefined
    }))
  };
}

export function WorkflowPanel({ instanceId, onRefresh }: Props) {
  const [detail, setDetail] = useState<WorkflowInstanceDetail | null>(null);
  const [visualGraph, setVisualGraph] = useState<VisualGraphDto | null>(null);
  const [selectedDmnId, setSelectedDmnId] = useState<string | null>(null);
  const [stepping, setStepping] = useState(false);

  useEffect(() => {
    const refreshData = () => {
      Promise.all([
        bpmnApi.getInstance(instanceId),
        bpmnApi.getGraph(instanceId)
      ])
        .then(([d, g]) => {
          setDetail(d);
          setVisualGraph(g);
        })
        .catch(console.error);
    };

    refreshData();

    const es = bpmnApi.subscribeToEvents(instanceId, () => {
      refreshData();
      onRefresh();
    });

    return () => {
      es.close();
    };
  }, [instanceId, onRefresh]);

  const handleNextStep = async () => {
    setStepping(true);
    try {
      await bpmnApi.nextStep(instanceId);
      const [d, g] = await Promise.all([
        bpmnApi.getInstance(instanceId),
        bpmnApi.getGraph(instanceId)
      ]);
      setDetail(d);
      setVisualGraph(g);
      onRefresh();
    } catch (e) {
      console.error(e);
    } finally {
      setStepping(false);
    }
  };

  const visitedNodes = useMemo(() => {
    if (!detail || !detail.nodes) return new Set<string>();
    const nodeIds = detail.nodes.map((n) => n.id);
    const currIdx = nodeIds.indexOf(detail.current_node);
    const complete = detail.status.includes("Completed");
    return new Set(
      detail.nodes.filter((_, idx) => idx < currIdx || complete).map((n) => n.id)
    );
  }, [detail]);

  const graphScene = useMemo(() => {
    if (!visualGraph || !detail) return null;
    return projectVisualGraph(
      visualGraph,
      detail.current_node,
      detail.status,
      visitedNodes
    );
  }, [visualGraph, detail, visitedNodes]);

  const handleCanvasAction = (action: any) => {
    if (action.type === "select_node") {
      const node = visualGraph?.nodes.find((n) => n.id === action.node_id);
      if (node?.kind === "business_rule_task" && node.plug?.startsWith("dmn-lite:")) {
        setSelectedDmnId(node.plug.replace("dmn-lite:", ""));
      } else {
        setSelectedDmnId(null);
      }
    }
  };

  if (!detail || !graphScene) return <div className="p-4 text-xs text-gray-500 font-mono">Loading workflow scene...</div>;

  const isWaiting =
    detail.status.includes("WaitingOnInvocation") ||
    detail.status.includes("WaitingOnSubmission");
  const isComplete = detail.status.includes("Completed");
  const isFailed = detail.status.includes("Failed");
  const currentNodeInfo = detail.nodes.find((n) => n.id === detail.current_node);

  return (
    <div className="flex flex-col h-full bg-gray-950 text-gray-100">
      {/* Top: Status & controls bar */}
      <div className="p-3 border-b border-gray-800 flex items-center justify-between shrink-0">
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
          <span className="text-xs text-gray-400">CBU: {detail.cbu_type}</span>
          <div className="flex gap-2 text-[10px] font-mono opacity-60">
            <span className="text-violet-300">■ ob-poc</span>
            <span className="text-cyan-300">■ dmn-lite</span>
          </div>
        </div>

        {/* Next step controls */}
        {(isWaiting || (!isComplete && !isFailed)) && !isComplete && (
          <button
            onClick={handleNextStep}
            disabled={stepping}
            className="text-xs bg-blue-700 hover:bg-blue-600 disabled:bg-gray-800 text-white px-3 py-1.5 rounded font-mono transition-colors shadow-md"
          >
            {stepping
              ? "Stepping…"
              : `▶ Next: ${currentNodeInfo?.label ?? detail.current_node}`}
          </button>
        )}
        {isComplete && (
          <span className="text-xs text-green-400 font-mono">✓ CBU Operational</span>
        )}
      </div>

      {/* Middle: 2D Workflow Canvas */}
      <div className="flex-1 min-h-[400px] bg-gray-900 relative">
        <ConstellationCanvas
          graphScene={graphScene}
          viewLevel="core"
          onAction={handleCanvasAction}
        />
      </div>

      {/* Variables variables viewer */}
      {Object.keys(detail.variables).length > 0 && (
        <div className="p-3 border-t border-gray-800 text-xs shrink-0 max-h-36 overflow-y-auto">
          <div className="text-gray-400 mb-1 font-mono">Variables</div>
          <pre className="bg-gray-950 p-2 rounded text-green-300 overflow-x-auto text-[10px]">
            {JSON.stringify(detail.variables, null, 2)}
          </pre>
        </div>
      )}

      {/* Bottom: DMN Table Drawer */}
      {selectedDmnId && (
        <div className="h-64 border-t border-gray-800 overflow-y-auto shrink-0 bg-gray-900">
          <DmnTableViewer decisionId={selectedDmnId} />
        </div>
      )}
    </div>
  );
}
