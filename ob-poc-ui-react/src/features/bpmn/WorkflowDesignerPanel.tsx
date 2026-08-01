import { useCallback, useEffect, useRef, useState } from "react";
import {
  bpmnTemplatesApi,
  buildBranchedWorkflowOps,
  missingArgumentNames,
  seedStartKey,
  waitingJobNodeId,
  type InstanceStatus,
  type PendingProposal,
  type SessionGraph,
  type UtteranceResponse,
} from "@/api/bpmnTemplates";
import { WorkflowGraphView } from "./WorkflowGraphView";

/**
 * Sage REPL workflow-designer panel — mounts inside the chat page's BPMN
 * workspace viewport. Owns a bpmn-lite DESIGNER design session bound to
 * the chat session (persisted in localStorage; the designer's store is
 * in-memory, so a stale id 404s and is transparently re-created), and
 * renders the COMPILED workflow via WorkflowGraphView so the user can
 * see their intention as the compiler understood it, Camunda-style.
 *
 * The DIR-002 propose→ratify→apply loop is now live end-to-end: an
 * utterance may return a dry-staged proposal (graph NOT yet changed),
 * rendered as a card with Ratify/Reject; ratifying appends the
 * GraphEdit and the compiled workflow re-fetch makes the mutation
 * visible. The seed button remains for the demo branched workflow
 * (2-way parallel split and merge, 8 tasks) via direct graph-edit,
 * and save/spawn/advance close the round trip against the real engine.
 */

interface Props {
  chatSessionId: string;
}

type Banner = { kind: "success" | "error"; text: string } | null;

const storageKey = (chatSessionId: string) => `bpmn.designer.session.${chatSessionId}`;

export function WorkflowDesignerPanel({ chatSessionId }: Props) {
  const [designSessionId, setDesignSessionId] = useState<string | null>(null);
  const [graph, setGraph] = useState<SessionGraph | null>(null);
  const [utterance, setUtterance] = useState("");
  // The anchor node for utterances — binding rule R1 takes only explicit
  // anchors, so positional proposals need one; set by clicking a graph node.
  const [anchorNodeId, setAnchorNodeId] = useState<string | null>(null);
  const [sageReply, setSageReply] = useState<UtteranceResponse | null>(null);
  const [pendingProposal, setPendingProposal] = useState<PendingProposal | null>(null);
  // Outcome note shown in place of / inside the proposal card:
  // green "applied" after a 200 ratify, red drift message after a 409.
  const [proposalNote, setProposalNote] = useState<
    { kind: "applied" | "drift"; text: string } | null
  >(null);
  const [showOps, setShowOps] = useState(false);
  const [templateName, setTemplateName] = useState("template1");
  const [saveInfo, setSaveInfo] = useState<string | null>(null);
  const [instanceId, setInstanceId] = useState<string | null>(null);
  const [status, setStatus] = useState<InstanceStatus | null>(null);
  const [banner, setBanner] = useState<Banner>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const cancelled = useRef(false);

  useEffect(() => {
    cancelled.current = false;
    return () => { cancelled.current = true; };
  }, []);

  const refreshGraph = useCallback(async (sid: string) => {
    setGraph(await bpmnTemplatesApi.sessionGraph(sid));
  }, []);

  // Bind (or re-create) the design session for this chat session.
  useEffect(() => {
    let alive = true;
    (async () => {
      setError(null);
      const stored = localStorage.getItem(storageKey(chatSessionId));
      if (stored) {
        try {
          const g = await bpmnTemplatesApi.sessionGraph(stored);
          if (!alive) return;
          setDesignSessionId(stored);
          setGraph(g);
          return;
        } catch {
          // Designer restarted (in-memory store) — fall through and re-create.
          localStorage.removeItem(storageKey(chatSessionId));
        }
      }
      try {
        const { session_id } = await bpmnTemplatesApi.createSession(
          `sage-${chatSessionId.slice(0, 8)}`
        );
        if (!alive) return;
        localStorage.setItem(storageKey(chatSessionId), session_id);
        setDesignSessionId(session_id);
        // Fresh sessions have no graph edits: /graph reports
        // compiles:false, which renders as the empty-workflow hint.
        const g = await bpmnTemplatesApi.sessionGraph(session_id);
        if (alive) setGraph(g);
      } catch (e) {
        if (alive) setError(String(e));
      }
    })();
    return () => { alive = false; };
  }, [chatSessionId]);

  const handleUtterance = async () => {
    if (!designSessionId || !utterance.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const reply = await bpmnTemplatesApi.sessionUtterance(
        designSessionId,
        utterance.trim(),
        anchorNodeId ?? undefined
      );
      setSageReply(reply);
      // One card at a time: a new utterance's proposal replaces the card.
      // The replaced proposal stays pending server-side, so fire-and-forget
      // reject it to keep the server's pending list in step with the UI —
      // its failure is logged, not surfaced (the card it belonged to is gone).
      if (pendingProposal && reply.proposal && reply.proposal.proposal_id !== pendingProposal.proposal_id) {
        bpmnTemplatesApi
          .rejectProposal(designSessionId, pendingProposal.proposal_id)
          .catch((e) => console.warn("reject of replaced proposal failed:", e));
      }
      setPendingProposal(reply.proposal ?? null);
      setProposalNote(null);
      setShowOps(false);
      setUtterance("");
      await refreshGraph(designSessionId);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleRatify = async () => {
    if (!designSessionId || !pendingProposal) return;
    setBusy(true);
    setError(null);
    try {
      await bpmnTemplatesApi.ratifyProposal(designSessionId, pendingProposal.proposal_id);
      setPendingProposal(null);
      setProposalNote({ kind: "applied", text: "Proposal applied — graph updated." });
      // The GraphEdit landed: re-fetch so the compiled workflow shows the mutation.
      await refreshGraph(designSessionId);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("-> 409")) {
        // Graph drifted since staging; the proposal is consumed server-side.
        setPendingProposal(null);
        setProposalNote({ kind: "drift", text: msg });
      } else {
        setError(msg);
      }
    } finally {
      setBusy(false);
    }
  };

  const handleReject = async () => {
    if (!designSessionId || !pendingProposal) return;
    setBusy(true);
    setError(null);
    try {
      await bpmnTemplatesApi.rejectProposal(designSessionId, pendingProposal.proposal_id);
      setPendingProposal(null);
      setProposalNote(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleSeed = async () => {
    if (!designSessionId) return;
    setBusy(true);
    setError(null);
    try {
      const startKey = await seedStartKey(designSessionId);
      const ops = buildBranchedWorkflowOps(startKey);
      await bpmnTemplatesApi.graphEdit(designSessionId, ops, "seed branched workflow (designer panel)");
      await refreshGraph(designSessionId);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleSave = async () => {
    if (!designSessionId) return;
    setBusy(true);
    setError(null);
    try {
      const r = await bpmnTemplatesApi.saveSession(designSessionId, templateName);
      setSaveInfo(`${r.template_name} v${r.template_version} — ${r.state}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleSpawnAndRun = async () => {
    setBusy(true);
    setError(null);
    setBanner(null);
    try {
      const spawned = await bpmnTemplatesApi.spawn(templateName);
      setInstanceId(spawned.instance_id);
      let prev = await bpmnTemplatesApi.instanceStatus(spawned.instance_id);
      setStatus(prev);
      for (let i = 0; i < 10; i++) {
        if (cancelled.current) return;
        await new Promise((r) => setTimeout(r, 400));
        const next = await bpmnTemplatesApi.advance(spawned.instance_id);
        setStatus(next);
        if (next.state === "Completed") {
          setBanner({ kind: "success", text: `Instance completed after ${i + 1} advance(s).` });
          return;
        }
        if (
          prev.state === next.state &&
          prev.waiting_jobs.map((j) => j.job_key).join(",") ===
            next.waiting_jobs.map((j) => j.job_key).join(",")
        ) {
          setBanner({ kind: "error", text: `No progress: stuck at ${next.state}.` });
          return;
        }
        prev = next;
      }
      setBanner({ kind: "error", text: "Did not complete within 10 advances." });
    } catch (e) {
      setBanner({ kind: "error", text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const hasNodes = Boolean(graph?.compiles && graph.graph && graph.graph.nodes.length > 1);

  return (
    <div className="flex flex-col gap-3 p-3 overflow-y-auto h-full">
      <div className="text-xs font-mono text-gray-500">
        designer session:{" "}
        <span className="text-gray-300">{designSessionId ?? "connecting…"}</span>
        {" · chat "}
        <span className="text-gray-400">{chatSessionId.slice(0, 8)}</span>
      </div>

      {error && (
        <div className="text-xs text-red-300 bg-red-950 border border-red-800 rounded p-2 font-mono whitespace-pre-wrap break-all">
          {error}
        </div>
      )}

      {/* Sage utterance against the design session */}
      <div className="flex gap-2">
        <input
          value={utterance}
          onChange={(e) => setUtterance(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleUtterance()}
          placeholder={
            anchorNodeId
              ? `tell Sage what to do at '${anchorNodeId}'…`
              : "click a graph node to anchor, then tell Sage…"
          }
          className="flex-1 text-xs bg-gray-900 border border-gray-700 rounded px-2 py-1.5 font-mono text-gray-200"
        />
        <button
          onClick={handleUtterance}
          disabled={busy || !designSessionId || !utterance.trim()}
          className="text-xs bg-gray-800 hover:bg-gray-700 disabled:opacity-50 px-3 py-1.5 rounded font-mono"
        >
          Utter
        </button>
      </div>
      {sageReply && (
        <div className="text-xs font-mono text-gray-400 bg-gray-900 border border-gray-800 rounded p-2 whitespace-pre-wrap">
          {sageReply.message}
          {missingArgumentNames(sageReply.disposition).length > 0 && (
            <div className="mt-1 text-amber-300">
              missing bindings: {missingArgumentNames(sageReply.disposition).join(", ")} — add them
              to the utterance.
            </div>
          )}
          {sageReply.proposal_refusal && (
            <div className="mt-1 text-red-300">proposal refused: {sageReply.proposal_refusal}</div>
          )}
        </div>
      )}

      {/* DIR-002 proposal card — dry-staged; graph mutates only on Ratify */}
      {pendingProposal && (
        <div className="text-xs font-mono bg-gray-900 border border-blue-900 rounded p-2 flex flex-col gap-2">
          <div className="text-gray-200">
            <span className="text-blue-300">proposal</span> {pendingProposal.description}
          </div>
          <button
            onClick={() => setShowOps((s) => !s)}
            className="self-start text-gray-500 hover:text-gray-300"
          >
            {showOps ? "▾ operations" : "▸ operations"}
          </button>
          {showOps && (
            <pre className="text-[10px] text-gray-400 bg-gray-950 border border-gray-800 rounded p-2 overflow-x-auto max-h-40 overflow-y-auto">
              {JSON.stringify(pendingProposal.operations, null, 2)}
            </pre>
          )}
          <div className="flex gap-2">
            <button
              onClick={handleRatify}
              disabled={busy}
              className="text-xs bg-green-900 hover:bg-green-800 disabled:opacity-50 px-3 py-1 rounded"
            >
              Ratify
            </button>
            <button
              onClick={handleReject}
              disabled={busy}
              className="text-xs bg-gray-800 hover:bg-gray-700 disabled:opacity-50 px-3 py-1 rounded"
            >
              Reject
            </button>
          </div>
        </div>
      )}
      {proposalNote && (
        <div
          className={`text-xs font-mono rounded p-2 border whitespace-pre-wrap break-all ${
            proposalNote.kind === "applied"
              ? "text-green-300 bg-green-950 border-green-800"
              : "text-red-300 bg-red-950 border-red-800"
          }`}
        >
          {proposalNote.text}
        </div>
      )}

      {/* Compiled workflow graph — the user's check that intention == compiled reality */}
      <div className="border border-gray-800 rounded p-2">
        <div className="text-xs font-semibold text-gray-300 mb-1">
          Compiled workflow
          {graph?.graph ? ` — ${graph.graph.workflow_id}` : ""}
          {status && status.state !== "Completed" && (
            <span className="ml-2 text-amber-400">
              ● token at {status.waiting_jobs.map(waitingJobNodeId).join(", ") || "—"}
            </span>
          )}
          {status?.state === "Completed" && <span className="ml-2 text-green-400">✓ completed</span>}
        </div>
        {graph ? (
          hasNodes ? (
            <WorkflowGraphView
              graph={graph}
              activeNodeIds={status?.waiting_jobs.map(waitingJobNodeId) ?? []}
              completed={status?.state === "Completed"}
              selectedNodeId={anchorNodeId}
              onSelectNode={(id) => setAnchorNodeId((cur) => (cur === id ? null : id))}
            />
          ) : (
            <div className="text-xs font-mono text-gray-500 p-2">
              Empty workflow — seed the demo chain to see the compiled graph.
            </div>
          )
        ) : (
          <div className="text-xs font-mono text-gray-600 p-2">loading…</div>
        )}
      </div>

      {/* Author / publish / run controls */}
      <div className="flex flex-wrap gap-2 items-center">
        {!hasNodes && (
          <button
            onClick={handleSeed}
            disabled={busy || !designSessionId}
            className="text-xs bg-gray-800 hover:bg-gray-700 disabled:opacity-50 px-3 py-1.5 rounded font-mono"
          >
            Seed branched workflow
          </button>
        )}
        {hasNodes && (
          <>
            <input
              value={templateName}
              onChange={(e) => setTemplateName(e.target.value)}
              className="text-xs bg-gray-900 border border-gray-700 rounded px-2 py-1.5 font-mono text-gray-200 w-32"
              placeholder="template name"
            />
            <button
              onClick={handleSave}
              disabled={busy || !templateName.trim()}
              className="text-xs bg-gray-800 hover:bg-gray-700 disabled:opacity-50 px-3 py-1.5 rounded font-mono"
            >
              Publish template
            </button>
            <button
              onClick={handleSpawnAndRun}
              disabled={busy || !saveInfo}
              className="text-xs bg-gray-800 hover:bg-gray-700 disabled:opacity-50 px-3 py-1.5 rounded font-mono"
            >
              Spawn + run to completion
            </button>
          </>
        )}
        {saveInfo && <span className="text-xs font-mono text-green-400">{saveInfo}</span>}
      </div>

      {instanceId && status && (
        <div className="text-xs font-mono text-gray-400">
          instance <span className="text-gray-200">{instanceId}</span> · state{" "}
          <span className={status.state === "Completed" ? "text-green-400" : "text-amber-300"}>
            {status.state}
          </span>
        </div>
      )}
      {banner && (
        <div
          className={`text-xs font-mono rounded p-2 border whitespace-pre-wrap break-all ${
            banner.kind === "success"
              ? "text-green-300 bg-green-950 border-green-800"
              : "text-red-300 bg-red-950 border-red-800"
          }`}
        >
          {banner.text}
        </div>
      )}
    </div>
  );
}
