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
export type Operation = {
  AppendNode: { anchor: string; key: string; node: IRNode; edge_id: string };
};

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

// ── The seven endpoints ──────────────────────────────────────────────────

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
