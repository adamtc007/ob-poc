/**
 * Designer-service API client (bpmn-lite-server-designer on port 8080).
 *
 * Covers the template round-trip: design session -> graph-edit -> save
 * (publish template) -> list published -> spawn -> status/advance.
 * Distinct from src/api/bpmn.ts, which targets the runner's /bpmn/* demo
 * endpoints.
 *
 * Proxied by Vite: /api/dsl/* and /bpmn/* both route to :8080.
 */

// ── RFC-4122 v5 UUID (SHA-1) ─────────────────────────────────────────────
//
// The designer seeds every session with a Start node whose NodeKey is
// uuid_v5(SEED_NAMESPACE, session_id_bytes) — replicated here so the
// client can anchor its first AppendNode on the seed Start.

/** Fixed namespace from bpmn-lite-server-designer/src/rest.rs seed_start_key. */
export const SEED_START_NAMESPACE = "b13f1a02-d299-4a71-9c3e-7a215c0e8b44";

function uuidToBytes(uuid: string): Uint8Array {
  const hex = uuid.replace(/-/g, "");
  if (hex.length !== 32 || /[^0-9a-fA-F]/.test(hex)) {
    throw new Error(`invalid uuid: ${uuid}`);
  }
  const bytes = new Uint8Array(16);
  for (let i = 0; i < 16; i++) {
    bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bytesToUuid(bytes: Uint8Array): string {
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/** RFC-4122 v5: SHA-1(namespace_bytes || name_bytes), version/variant bits set. */
export async function uuidV5(namespace: string, nameBytes: Uint8Array): Promise<string> {
  const ns = uuidToBytes(namespace);
  const input = new Uint8Array(ns.length + nameBytes.length);
  input.set(ns, 0);
  input.set(nameBytes, ns.length);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-1", input));
  const out = digest.slice(0, 16);
  out[6] = (out[6] & 0x0f) | 0x50; // version 5
  out[8] = (out[8] & 0x3f) | 0x80; // RFC-4122 variant
  return bytesToUuid(out);
}

/** The deterministic NodeKey of a session's seeded Start node. */
export function seedStartKey(sessionId: string): Promise<string> {
  return uuidV5(SEED_START_NAMESPACE, uuidToBytes(sessionId));
}

// ── Wire types ───────────────────────────────────────────────────────────

/** bpmn_lite_compiler::IRNode — externally tagged serde enum (subset used here). */
export type IRNode =
  | { ServiceTask: { id: string; name: string; task_type: string } }
  | { End: { id: string; terminate: boolean } };

/** designer_graph::ops::Operation — externally tagged serde enum (subset used here). */
export type Operation =
  | { AppendNode: { anchor: string; key: string; node: IRNode; edge_id: string } }
  | { InsertAfter: { anchor: string; key: string; node: IRNode; edge_id: string } }
  | {
      CreateParallelRegion: {
        anchor: string;
        fork_key: string;
        fork_node_id: string;
        join_key: string;
        join_node_id: string;
        entry_edge_id: string;
        branches: {
          key: string;
          node: IRNode;
          in_edge_id: string;
          out_edge_id: string;
          condition: null;
        }[];
      };
    };

/**
 * The demo seed: a SESE parallel block with a 2-way branch and merge —
 * start → t1 → split ⇉ (t2a→t3a | t2b→t3b) ⇉ join → t4 → t5 → end.
 * CreateParallelRegion inserts fork+join CLOSED in one op (SESE by
 * construction); InsertAfter extends each branch. One graph-edit call
 * carries all 7 ops; admit() runs once over the staged sequence.
 */
export function buildBranchedWorkflowOps(startKey: string): Operation[] {
  const [t1, fork, join, t2a, t2b, t3a, t3b, t4, t5, end] = Array.from(
    { length: 10 },
    () => crypto.randomUUID()
  );
  const task = (id: string): IRNode => ({
    ServiceTask: { id, name: id, task_type: "noop" },
  });
  return [
    { AppendNode: { anchor: startKey, key: t1, node: task("t1"), edge_id: "f1" } },
    {
      CreateParallelRegion: {
        anchor: t1,
        fork_key: fork,
        fork_node_id: "split",
        join_key: join,
        join_node_id: "join",
        entry_edge_id: "f2",
        branches: [
          { key: t2a, node: task("t2a"), in_edge_id: "f3a", out_edge_id: "f4a", condition: null },
          { key: t2b, node: task("t2b"), in_edge_id: "f3b", out_edge_id: "f4b", condition: null },
        ],
      },
    },
    { InsertAfter: { anchor: t2a, key: t3a, node: task("t3a"), edge_id: "f5a" } },
    { InsertAfter: { anchor: t2b, key: t3b, node: task("t3b"), edge_id: "f5b" } },
    { AppendNode: { anchor: join, key: t4, node: task("t4"), edge_id: "f6" } },
    { AppendNode: { anchor: t4, key: t5, node: task("t5"), edge_id: "f7" } },
    { AppendNode: { anchor: t5, key: end, node: { End: { id: "end", terminate: false } }, edge_id: "f8" } },
  ];
}

/** Node id for a waiting job: multi-fiber rounds report node_id as "" —
 *  derive it from the job key's `instance:node:seq:idx` shape instead. */
export function waitingJobNodeId(job: WaitingJob): string {
  return job.node_id || job.job_key.split(":")[1] || "";
}

export interface CreateSessionResponse {
  session_id: string;
  [k: string]: unknown;
}

export interface GraphEditResponse {
  [k: string]: unknown;
}

export interface SaveSessionResponse {
  template_name: string;
  version: number;
  plan_hash: string;
  template_version: number;
  bytecode_version: string;
  state: string;
}

export interface PublishedTemplate {
  template_key: string;
  template_version: number;
  process_key: string;
  task_manifest: string[];
  created_at: number;
  published_at: number | null;
}

export interface SpawnResponse {
  instance_id: string;
  template_key: string;
  template_version: number;
  bytecode_version: string;
}

export interface WaitingJob {
  job_key: string;
  node_id: string;
}

export interface InstanceStatus {
  instance_id: string;
  state: string;
  waiting_jobs: WaitingJob[];
  fiber_count: number;
  wait_count: number;
}

// ── Session graph (the COMPILED workflow, server-laid-out) ───────────────
//
// GET /api/dsl/sessions/:id/graph — for graph-backed sessions the server
// runs the same admitted DAG -> to_ir -> project_ir chain the save/spawn
// path uses, so this graph IS what an instance executes. layout maps
// node id -> {x, y} with x = execution depth (left-to-right flow order).

export interface VisualNode {
  id: string;
  label: string;
  kind: string; // "start" | "end" | "task" | gateway kinds
  plug: string | null;
}

export interface VisualEdge {
  from: string;
  to: string;
  condition: string | null;
}

export interface SessionGraph {
  compiles: boolean;
  diagnostics: string[];
  graph: {
    workflow_id: string;
    nodes: VisualNode[];
    edges: VisualEdge[];
  } | null;
  layout: Record<string, { x: number; y: number }> | null;
  source_hash: string;
}

/** DIR-002 dry-staged pending proposal — the graph is NOT yet changed. */
export interface PendingProposal {
  proposal_id: string;
  operations: unknown;
  description: string;
}

export interface UtteranceResponse {
  seq: number;
  message: string;
  disposition: unknown;
  capture: string;
  dev_capture: string;
  /** DIR-002: staged proposal awaiting human ratify/reject (null when none). */
  proposal?: PendingProposal | null;
  proposal_refusal?: string | null;
}

/**
 * Extract missing-binding names from a MissingArguments disposition.
 * Dispositions are externally tagged serde enums; the MissingArguments
 * payload names the bindings that could not be derived from the utterance.
 * Tolerant of payload shape (string array, or object whose values contain
 * the names) — returns [] when the disposition is anything else.
 */
export function missingArgumentNames(disposition: unknown): string[] {
  if (typeof disposition !== "object" || disposition === null) return [];
  const payload = (disposition as Record<string, unknown>)["MissingArguments"];
  if (payload === undefined) return [];
  const names: string[] = [];
  const walk = (v: unknown) => {
    if (typeof v === "string") names.push(v);
    else if (Array.isArray(v)) v.forEach(walk);
    else if (typeof v === "object" && v !== null) Object.values(v).forEach(walk);
  };
  walk(payload);
  return names;
}

// ── Fetch plumbing — every failure carries the server's error body ───────

async function jsonFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(path, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  const text = await res.text();
  if (!res.ok) {
    throw new Error(`${options?.method ?? "GET"} ${path} -> ${res.status}: ${text}`);
  }
  return JSON.parse(text) as T;
}

// ── The endpoints ──────────────────────────────────────────────────

export const bpmnTemplatesApi = {
  createSession: (name: string) =>
    jsonFetch<CreateSessionResponse>("/api/dsl/sessions", {
      method: "POST",
      body: JSON.stringify({ name }),
    }),

  graphEdit: (sessionId: string, operations: Operation[], note: string) =>
    jsonFetch<GraphEditResponse>(`/api/dsl/sessions/${sessionId}/graph-edit`, {
      method: "POST",
      body: JSON.stringify({ operations, note }),
    }),

  saveSession: (sessionId: string, templateName: string) =>
    jsonFetch<SaveSessionResponse>(`/api/dsl/sessions/${sessionId}/save`, {
      method: "POST",
      body: JSON.stringify({ template_name: templateName }),
    }),

  sessionGraph: (sessionId: string) =>
    jsonFetch<SessionGraph>(`/api/dsl/sessions/${sessionId}/graph`),

  /** Sage utterance against a design session (shadow disposition pipeline).
   *  `anchor` is the BPMN id of the graph node the utterance is issued
   *  from — binding rule R1 takes ONLY explicit anchors (never inferred),
   *  so positional proposals require one; without it the board has no
   *  position and the utterance typically lands OutOfScope. */
  sessionUtterance: (sessionId: string, text: string, anchor?: string) =>
    jsonFetch<UtteranceResponse>(`/api/dsl/sessions/${sessionId}/utterance`, {
      method: "POST",
      body: JSON.stringify(anchor ? { text, anchor } : { text }),
    }),

  /** Ratify a staged DIR-002 proposal — 200 applies the graph-edit;
   *  409 = graph drifted since staging (proposal consumed); 404 unknown.
   *  Errors carry the server body via jsonFetch. */
  ratifyProposal: (sessionId: string, proposalId: string) =>
    jsonFetch<Record<string, unknown>>(
      `/api/dsl/sessions/${sessionId}/proposals/${proposalId}/ratify`,
      { method: "POST" }
    ),

  rejectProposal: (sessionId: string, proposalId: string) =>
    jsonFetch<Record<string, unknown>>(
      `/api/dsl/sessions/${sessionId}/proposals/${proposalId}/reject`,
      { method: "POST" }
    ),

  listProposals: (sessionId: string) =>
    jsonFetch<PendingProposal[]>(`/api/dsl/sessions/${sessionId}/proposals`),

  listPublished: () =>
    jsonFetch<PublishedTemplate[]>("/bpmn/templates/published"),

  spawn: (name: string, version?: number, payload?: unknown) =>
    jsonFetch<SpawnResponse>(`/bpmn/templates/${encodeURIComponent(name)}/spawn`, {
      method: "POST",
      body: JSON.stringify({ version, payload }),
    }),

  instanceStatus: (instanceId: string) =>
    jsonFetch<InstanceStatus>(`/bpmn/instances/${instanceId}/status`),

  advance: (instanceId: string) =>
    jsonFetch<InstanceStatus>(`/bpmn/instances/${instanceId}/advance`, {
      method: "POST",
    }),
};
