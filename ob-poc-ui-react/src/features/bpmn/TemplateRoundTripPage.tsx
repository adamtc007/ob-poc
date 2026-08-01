import { useCallback, useEffect, useRef, useState } from "react";
import {
  bpmnTemplatesApi,
  buildBranchedWorkflowOps,
  seedStartKey,
  waitingJobNodeId,
  type InstanceStatus,
  type PublishedTemplate,
  type SaveSessionResponse,
  type SessionGraph,
} from "@/api/bpmnTemplates";
import { WorkflowGraphView } from "./WorkflowGraphView";

/**
 * Template round-trip harness against the designer service (:8080):
 * build a branched workflow (2-way split/merge) via graph-edit, publish it as a template,
 * spawn an instance, and drive it to completion via status/advance.
 */

type Banner = { kind: "success" | "error"; text: string } | null;

export function TemplateRoundTripPage() {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [opsResult, setOpsResult] = useState<string | null>(null);
  const [sessionGraph, setSessionGraph] = useState<SessionGraph | null>(null);
  const [creating, setCreating] = useState(false);

  const [templateName, setTemplateName] = useState("template1");
  const [saving, setSaving] = useState(false);
  const [saveResult, setSaveResult] = useState<SaveSessionResponse | null>(null);

  const [published, setPublished] = useState<PublishedTemplate[]>([]);
  const [listError, setListError] = useState<string | null>(null);

  const [instanceId, setInstanceId] = useState<string | null>(null);
  const [status, setStatus] = useState<InstanceStatus | null>(null);
  const [running, setRunning] = useState(false);
  const [banner, setBanner] = useState<Banner>(null);
  const [error, setError] = useState<string | null>(null);
  const cancelled = useRef(false);

  useEffect(() => {
    // StrictMode runs this cleanup once at its simulated unmount; the ref
    // survives the remount, so it must be re-armed in the effect body.
    cancelled.current = false;
    return () => { cancelled.current = true; };
  }, []);

  const refreshPublished = useCallback(async () => {
    try {
      setPublished(await bpmnTemplatesApi.listPublished());
      setListError(null);
    } catch (e) {
      setListError(String(e));
    }
  }, []);

  useEffect(() => {
    refreshPublished();
  }, [refreshPublished]);

  const handleCreate = async () => {
    setCreating(true);
    setError(null);
    setOpsResult(null);
    try {
      const name = `wf-roundtrip-${Date.now()}`;
      const { session_id } = await bpmnTemplatesApi.createSession(name);
      setSessionId(session_id);

      const startKey = await seedStartKey(session_id);
      const ops = buildBranchedWorkflowOps(startKey);
      const result = await bpmnTemplatesApi.graphEdit(session_id, ops, "build branched workflow");
      setOpsResult(JSON.stringify(result));
      // The compiled-workflow graph, straight from the DAG the server
      // admitted — what the user sees IS what an instance will execute.
      setSessionGraph(await bpmnTemplatesApi.sessionGraph(session_id));
    } catch (e) {
      setError(String(e));
    } finally {
      setCreating(false);
    }
  };

  const handleSave = async () => {
    if (!sessionId) return;
    setSaving(true);
    setError(null);
    try {
      setSaveResult(await bpmnTemplatesApi.saveSession(sessionId, templateName));
      await refreshPublished();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleSpawn = async (name: string, version: number) => {
    setError(null);
    setBanner(null);
    setStatus(null);
    try {
      const spawned = await bpmnTemplatesApi.spawn(name, version);
      setInstanceId(spawned.instance_id);
      setStatus(await bpmnTemplatesApi.instanceStatus(spawned.instance_id));
    } catch (e) {
      setError(String(e));
    }
  };

  const handleAdvance = async () => {
    if (!instanceId) return;
    setError(null);
    try {
      setStatus(await bpmnTemplatesApi.advance(instanceId));
    } catch (e) {
      setError(String(e));
    }
  };

  const handleRunToCompletion = async () => {
    if (!instanceId) return;
    setRunning(true);
    setError(null);
    setBanner(null);
    try {
      let prev = status;
      for (let i = 0; i < 10; i++) {
        if (cancelled.current) return;
        const next = await bpmnTemplatesApi.advance(instanceId);
        setStatus(next);
        if (next.state === "Completed") {
          setBanner({ kind: "success", text: `Instance ${instanceId} completed after ${i + 1} advance(s).` });
          return;
        }
        if (
          prev &&
          prev.state === next.state &&
          prev.fiber_count === next.fiber_count &&
          prev.wait_count === next.wait_count &&
          prev.waiting_jobs.map((j) => j.job_key).join(",") ===
            next.waiting_jobs.map((j) => j.job_key).join(",")
        ) {
          setBanner({ kind: "error", text: `No progress after advance ${i + 1}: state stuck at ${next.state}.` });
          return;
        }
        prev = next;
        await new Promise((r) => setTimeout(r, 300));
      }
      setBanner({ kind: "error", text: "Did not complete within 10 advances." });
    } catch (e) {
      setBanner({ kind: "error", text: String(e) });
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="h-full overflow-y-auto bg-gray-950 text-gray-100 p-6">
      <div className="max-w-3xl mx-auto flex flex-col gap-4">
        <h1 className="text-sm font-semibold text-gray-300">
          Template round-trip (designer service :8080)
        </h1>

        {error && (
          <div className="text-xs text-red-300 bg-red-950 border border-red-800 rounded p-2 font-mono whitespace-pre-wrap break-all">
            {error}
          </div>
        )}

        {/* Step 1: session + graph */}
        <section className="border border-gray-800 rounded p-3">
          <div className="text-xs font-semibold text-gray-300 mb-2">1 — Build workflow</div>
          <button
            onClick={handleCreate}
            disabled={creating}
            className="text-xs bg-gray-800 hover:bg-gray-700 disabled:opacity-50 px-3 py-1.5 rounded font-mono transition-colors"
          >
            {creating ? "Creating…" : "Create branched workflow"}
          </button>
          {sessionId && (
            <div className="mt-2 text-xs font-mono text-gray-400">
              session: <span className="text-gray-200">{sessionId}</span>
            </div>
          )}
          {opsResult && (
            <div className="mt-1 text-xs font-mono text-gray-500 break-all">ops result: {opsResult}</div>
          )}
        </section>

        {/* Compiled workflow graph — Camunda-style flow view in execution order */}
        {sessionGraph && (
          <section className="border border-gray-800 rounded p-3">
            <div className="text-xs font-semibold text-gray-300 mb-2">
              Workflow graph (compiled
              {sessionGraph.graph ? ` — ${sessionGraph.graph.workflow_id}` : ""})
              {status && !["Completed"].includes(status.state) && (
                <span className="ml-2 text-amber-400">● token at {status.waiting_jobs.map(waitingJobNodeId).join(", ") || "—"}</span>
              )}
              {status?.state === "Completed" && (
                <span className="ml-2 text-green-400">✓ completed</span>
              )}
            </div>
            <WorkflowGraphView
              graph={sessionGraph}
              activeNodeIds={status?.waiting_jobs.map(waitingJobNodeId) ?? []}
              completed={status?.state === "Completed"}
            />
          </section>
        )}

        {/* Step 2: save as template */}
        <section className="border border-gray-800 rounded p-3">
          <div className="text-xs font-semibold text-gray-300 mb-2">2 — Save as template</div>
          <div className="flex gap-2 items-center">
            <input
              value={templateName}
              onChange={(e) => setTemplateName(e.target.value)}
              className="text-xs font-mono bg-gray-900 border border-gray-700 rounded px-2 py-1.5 flex-1"
              placeholder="template name"
            />
            <button
              onClick={handleSave}
              disabled={saving || !sessionId || !templateName}
              className="text-xs bg-gray-800 hover:bg-gray-700 disabled:opacity-50 px-3 py-1.5 rounded font-mono transition-colors"
            >
              {saving ? "Saving…" : "Save as template"}
            </button>
          </div>
          {saveResult && (
            <div className="mt-2 text-xs font-mono text-gray-400">
              {saveResult.template_name} v{saveResult.template_version} — {saveResult.state} (bytecode{" "}
              {saveResult.bytecode_version})
            </div>
          )}
        </section>

        {/* Step 3: published templates */}
        <section className="border border-gray-800 rounded p-3">
          <div className="flex justify-between items-center mb-2">
            <div className="text-xs font-semibold text-gray-300">3 — Published templates</div>
            <button
              onClick={refreshPublished}
              className="text-xs text-gray-500 hover:text-gray-300 font-mono"
            >
              refresh
            </button>
          </div>
          {listError && (
            <div className="text-xs text-red-300 font-mono whitespace-pre-wrap break-all">{listError}</div>
          )}
          {published.length === 0 && !listError && (
            <div className="text-xs text-gray-600">None published yet</div>
          )}
          {published.map((t) => (
            <div
              key={`${t.template_key}:${t.template_version}`}
              className="flex justify-between items-center text-xs font-mono py-1 border-b border-gray-900 last:border-0"
            >
              <span>
                {t.template_key} v{t.template_version}{" "}
                <span className="text-gray-600">({t.task_manifest.join(", ") || "no tasks"})</span>
              </span>
              <button
                onClick={() => handleSpawn(t.template_key, t.template_version)}
                className="bg-gray-800 hover:bg-gray-700 px-2 py-1 rounded transition-colors"
              >
                Spawn instance
              </button>
            </div>
          ))}
        </section>

        {/* Step 4: instance panel */}
        <section className="border border-gray-800 rounded p-3">
          <div className="text-xs font-semibold text-gray-300 mb-2">4 — Instance</div>
          {!instanceId && <div className="text-xs text-gray-600">No instance spawned yet</div>}
          {instanceId && (
            <>
              <div className="text-xs font-mono text-gray-400">
                instance: <span className="text-gray-200">{instanceId}</span>
              </div>
              {status && (
                <div className="mt-2 text-xs font-mono text-gray-400 flex flex-col gap-0.5">
                  <div>
                    state:{" "}
                    <span
                      className={
                        status.state === "Completed"
                          ? "text-green-400"
                          : status.state === "Running"
                          ? "text-yellow-400"
                          : "text-gray-200"
                      }
                    >
                      {status.state}
                    </span>
                  </div>
                  <div>
                    fibers: {status.fiber_count} · waits: {status.wait_count}
                  </div>
                  <div>
                    waiting jobs:{" "}
                    {status.waiting_jobs.length === 0
                      ? "none"
                      : status.waiting_jobs.map((j) => `${j.node_id} (${j.job_key})`).join(", ")}
                  </div>
                </div>
              )}
              <div className="mt-2 flex gap-2">
                <button
                  onClick={handleAdvance}
                  disabled={running}
                  className="text-xs bg-gray-800 hover:bg-gray-700 disabled:opacity-50 px-3 py-1.5 rounded font-mono transition-colors"
                >
                  Advance
                </button>
                <button
                  onClick={handleRunToCompletion}
                  disabled={running}
                  className="text-xs bg-blue-900 hover:bg-blue-800 disabled:opacity-50 px-3 py-1.5 rounded font-mono transition-colors"
                >
                  {running ? "Running…" : "Run to completion"}
                </button>
              </div>
              {banner && (
                <div
                  className={`mt-2 text-xs font-mono rounded p-2 border whitespace-pre-wrap break-all ${
                    banner.kind === "success"
                      ? "text-green-300 bg-green-950 border-green-800"
                      : "text-red-300 bg-red-950 border-red-800"
                  }`}
                >
                  {banner.text}
                </div>
              )}
            </>
          )}
        </section>
      </div>
    </div>
  );
}
