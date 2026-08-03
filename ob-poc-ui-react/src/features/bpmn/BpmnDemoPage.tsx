import { useCallback, useEffect, useState } from "react";
import { bpmnApi } from "@/api/bpmn";
import type { SageReasoningRecord, WorkflowInstanceSummary } from "@/api/bpmn";
import { WorkflowPanel } from "./WorkflowPanel";
import { SagePanel } from "./SagePanel";
import { PlanFeedPanel } from "./PlanFeedPanel";
import { WorkflowDesignerPanel } from "./WorkflowDesignerPanel";

type CbuType = "fund" | "corporate" | "trust";
type Panel = "designer" | "workflow" | "sage" | "feed";

interface Props {
  /** Chat/REPL session id — when present, the Designer tab binds a
   *  bpmn-lite design session to it (Sage REPL workflow designer). */
  chatSessionId?: string;
}

export function BpmnDemoPage({ chatSessionId }: Props) {
  const [instances, setInstances] = useState<WorkflowInstanceSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [sageRecords, setSageRecords] = useState<SageReasoningRecord[]>([]);
  const [activePanel, setActivePanel] = useState<Panel>(chatSessionId ? "designer" : "workflow");
  const [starting, setStarting] = useState(false);
  // Service-identity health contract: both the runner and the designer
  // answer /bpmn/health with {status, service}; we dispatch on `service`
  // to pick the surface. The old check treated a 404 as "server down",
  // so the NORMAL designer deployment permanently showed an alarming
  // "runner unreachable" banner — mode detection by absence.
  const [serverMode, setServerMode] = useState<
    "runner" | "designer" | "down" | null
  >(null);
  const [tier1Bundle, setTier1Bundle] = useState<string | null>(null);

  useEffect(() => {
    bpmnApi
      .health()
      .then((h) => {
        if (h.service === "bpmn-lite-designer") {
          setServerMode("designer");
          setTier1Bundle(h.tier1_bundle ?? null);
        } else {
          setServerMode("runner");
        }
      })
      .catch(() => setServerMode("down"));
  }, []);

  const refresh = useCallback(() => {
    bpmnApi.listInstances().then(setInstances).catch(console.error);
    if (selectedId) {
      bpmnApi.getSage(selectedId).then(setSageRecords).catch(console.error);
    }
  }, [selectedId]);

  useEffect(() => {
    // The instance-list poll targets the RUNNER's demo endpoints; against
    // the designer (or nothing) it would just 404 every two seconds.
    if (serverMode !== "runner") return;
    refresh();
    const id = setInterval(refresh, 2000);
    return () => clearInterval(id);
  }, [refresh, serverMode]);

  const handleStart = async (cbuType: CbuType) => {
    setStarting(true);
    try {
      const { instance_id } = await bpmnApi.startInstance(cbuType);
      setSelectedId(instance_id);
      refresh();
    } catch (e) {
      console.error(e);
    } finally {
      setStarting(false);
    }
  };

  const handleReset = async () => {
    await bpmnApi.reset();
    setSelectedId(null);
    setInstances([]);
    setSageRecords([]);
  };

  // The designer service IS the expected deployment for this workspace —
  // a calm identity line, not an alarm. The runner demo surface (fund/
  // corporate/trust instances below) only renders against the runner.
  if (serverMode === "designer" && chatSessionId) {
    return (
      <div className="flex flex-col h-full bg-gray-950 text-gray-100">
        <div className="text-xs text-gray-500 bg-gray-900 border-b border-gray-800 px-3 py-1.5 font-mono">
          <span className="text-emerald-500">●</span> designer service
          {tier1Bundle ? (
            <span className="ml-2 text-gray-600">
              tier-1 SLM {tier1Bundle.slice(0, 40)}…
            </span>
          ) : (
            <span className="ml-2 text-amber-600">tier-0 serving (no SLM bundle loaded)</span>
          )}
        </div>
        <WorkflowDesignerPanel chatSessionId={chatSessionId} />
      </div>
    );
  }
  if (serverMode === "down") {
    // Genuinely nothing behind /bpmn — this one IS an error.
    if (chatSessionId) {
      return (
        <div className="flex flex-col h-full bg-gray-950 text-gray-100">
          <div className="text-xs text-red-400 bg-gray-900 border-b border-gray-800 px-3 py-1.5 font-mono">
            no bpmn service reachable at /bpmn — designer calls will fail until one is started
          </div>
          <WorkflowDesignerPanel chatSessionId={chatSessionId} />
        </div>
      );
    }
    return (
      <div className="flex items-center justify-center h-full text-sm text-red-400">
        bpmn-lite REST server not reachable at /bpmn — start with{" "}
        <code className="ml-1 font-mono bg-gray-800 px-1 rounded">
          BPMN_LITE_STORE=memory cargo x bpmn-lite start
        </code>
      </div>
    );
  }

  return (
    <div className="flex h-full overflow-hidden bg-gray-950 text-gray-100">
      {/* Sidebar: instance list + controls */}
      <div className="w-64 border-r border-gray-800 flex flex-col">
        <div className="p-3 border-b border-gray-800">
          <div className="text-xs font-semibold text-gray-300 mb-2">Start Demo</div>
          <div className="flex flex-col gap-1">
            {(["fund", "corporate", "trust"] as CbuType[]).map((t) => (
              <button
                key={t}
                onClick={() => handleStart(t)}
                disabled={starting}
                className="text-xs bg-gray-800 hover:bg-gray-700 disabled:opacity-50 px-2 py-1.5 rounded text-left font-mono capitalize transition-colors"
              >
                ▶ {t} CBU
              </button>
            ))}
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-2">
          <div className="text-xs text-gray-500 px-1 mb-1">Instances</div>
          {instances.length === 0 && (
            <div className="text-xs text-gray-600 px-1">None yet</div>
          )}
          {instances.map((inst) => (
            <button
              key={inst.id}
              onClick={() => setSelectedId(inst.id)}
              className={`w-full text-left text-xs px-2 py-1.5 rounded mb-1 font-mono transition-colors ${
                selectedId === inst.id
                  ? "bg-blue-900 text-blue-100"
                  : "hover:bg-gray-800 text-gray-300"
              }`}
            >
              <div className="truncate">{inst.cbu_type} CBU</div>
              <div
                className={`text-xs mt-0.5 ${
                  inst.status.includes("Completed")
                    ? "text-green-400"
                    : inst.status.includes("Failed")
                    ? "text-red-400"
                    : inst.status.includes("WaitingOn")
                    ? "text-yellow-400"
                    : "text-gray-500"
                }`}
              >
                {inst.current_node}
              </div>
            </button>
          ))}
        </div>

        <div className="p-2 border-t border-gray-800">
          <button
            onClick={handleReset}
            className="w-full text-xs text-gray-500 hover:text-red-400 py-1 transition-colors"
          >
            Reset demo
          </button>
        </div>
      </div>

      {/* Main panel area */}
      <div className="flex-1 flex flex-col min-w-0">
        {(() => {
          const tabs: { id: Panel; label: string }[] = [
            ...(chatSessionId ? [{ id: "designer" as Panel, label: "Designer" }] : []),
            ...(selectedId
              ? [
                  { id: "workflow" as Panel, label: "Workflow" },
                  { id: "sage" as Panel, label: `Sage (${sageRecords.length})` },
                  { id: "feed" as Panel, label: "Plan Feed" },
                ]
              : []),
          ];
          if (tabs.length === 0) {
            return (
              <div className="flex-1 flex items-center justify-center text-sm text-gray-500">
                Select an instance or start a new demo run.
              </div>
            );
          }
          const active = tabs.some((t) => t.id === activePanel) ? activePanel : tabs[0].id;
          return (
            <>
              {/* Panel tabs */}
              <div className="flex border-b border-gray-800 px-4">
                {tabs.map((tab) => (
                  <button
                    key={tab.id}
                    onClick={() => setActivePanel(tab.id)}
                    className={`text-xs px-4 py-2 font-mono transition-colors border-b-2 ${
                      active === tab.id
                        ? "border-blue-500 text-blue-300"
                        : "border-transparent text-gray-500 hover:text-gray-300"
                    }`}
                  >
                    {tab.label}
                  </button>
                ))}
              </div>

              {/* Active panel */}
              <div className="flex-1 overflow-y-auto">
                {active === "designer" && chatSessionId && (
                  <WorkflowDesignerPanel chatSessionId={chatSessionId} />
                )}
                {active === "workflow" && selectedId && (
                  <WorkflowPanel instanceId={selectedId} onRefresh={refresh} />
                )}
                {active === "sage" && <SagePanel records={sageRecords} />}
                {active === "feed" && selectedId && <PlanFeedPanel instanceId={selectedId} />}
              </div>
            </>
          );
        })()}
      </div>
    </div>
  );
}
