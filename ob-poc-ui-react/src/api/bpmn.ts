/**
 * BPMN Demo API client.
 * Talks to the bpmn-lite REST server at /bpmn/* (proxied by Vite to port 8080).
 */

const BPMN_BASE = "/bpmn";

export interface WorkflowInstanceSummary {
  id: string;
  workflow_id: string;
  current_node: string;
  status: string;
  cbu_type: string;
}

export interface SageReasoningRecord {
  id: string;
  execution_id: string;
  actor: string;
  mode: string;
  verb_fqn: string;
  outcome_class: string;
  context_snapshot: Record<string, unknown>;
  options_considered: { verb: string; score: number; reason: string }[];
  chosen: string;
  rationale: string;
  confidence: number;
  recorded_at: string;
}

export interface NodeInfo {
  id: string;
  label: string;
  /** Namespaced FQN e.g. "ob-poc:cbu.create" — present on cross-domain callouts. */
  fqn: string | null;
  /** Target domain e.g. "ob-poc" or "dmn-lite" — present on cross-domain callouts. */
  target_domain: string | null;
  kind: "start" | "end" | "gateway" | "service_task" | "business_rule_task";
}

export interface WorkflowInstanceDetail {
  id: string;
  workflow_id: string;
  current_node: string;
  status: string;
  variables: Record<string, unknown>;
  cbu_type: string;
  nodes: NodeInfo[];
  sage_records: SageReasoningRecord[];
}

async function bpmnFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${BPMN_BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`BPMN API ${res.status}: ${text}`);
  }
  return res.json() as Promise<T>;
}

export const bpmnApi = {
  health: () => bpmnFetch<{ status: string }>("/health"),

  listInstances: () =>
    bpmnFetch<WorkflowInstanceSummary[]>("/instances"),

  getInstance: (id: string) =>
    bpmnFetch<WorkflowInstanceDetail>(`/instances/${id}`),

  getSage: (id: string) =>
    bpmnFetch<SageReasoningRecord[]>(`/instances/${id}/sage`),

  startInstance: (cbuType: "fund" | "corporate" | "trust") =>
    bpmnFetch<{ instance_id: string }>("/instances/start", {
      method: "POST",
      body: JSON.stringify({ cbu_type: cbuType }),
    }),

  nextStep: (id: string) =>
    bpmnFetch<{ execution_id: string; node: string; message: string }>(
      `/instances/${id}/next-step`,
      { method: "POST" }
    ),

  reset: () =>
    fetch(`${BPMN_BASE}/instances`, { method: "DELETE" }).then(() => undefined),

  getGraph: (id: string) =>
    bpmnFetch<VisualGraphDto>(`/instances/${id}/graph`),

  getStack: (id: string) =>
    bpmnFetch<CallStackFrameDto[]>(`/instances/${id}/stack`),

  getDmnDecision: (decisionId: string) =>
    fetch(`/dmn/decisions/${decisionId}`, {
      headers: { "Content-Type": "application/json" },
    }).then((res) => {
      if (!res.ok) throw new Error(`DMN API ${res.status}`);
      return res.json() as Promise<DmnSchemaDto>;
    }),

  /** Subscribe to SSE lifecycle events for a process instance. */
  subscribeToEvents: (
    instanceId: string,
    onEvent: (data: unknown) => void,
    onError?: (e: Event) => void
  ): EventSource => {
    const es = new EventSource(`${BPMN_BASE}/instances/${instanceId}/events`);
    es.onmessage = (e) => {
      try {
        onEvent(JSON.parse(e.data));
      } catch {
        onEvent(e.data);
      }
    };
    if (onError) es.onerror = onError;
    return es;
  },
};

export interface VisualNodeDto {
  id: string;
  label: string;
  kind: "start" | "end" | "task" | "split" | "join" | "loop";
  plug: string | null;
  span: {
    start_offset: number;
    end_offset: number;
    start_line: number;
    start_col: number;
  } | null;
}

export interface VisualEdgeDto {
  from: string;
  to: string;
  condition: string | null;
}

export interface VisualGraphDto {
  workflow_id: string;
  nodes: VisualNodeDto[];
  edges: VisualEdgeDto[];
}

export interface CallStackFrameDto {
  instance_id: string;
  workflow_id: string;
  node_id: string;
  plug: string | null;
  span: VisualNodeDto["span"];
  status: string;
}

export interface DmnRuleInputCell {
  op: string;
  value: string;
}

export interface DmnRuleDto {
  id: string;
  inputs: DmnRuleInputCell[];
  outputs: string[];
}

export interface DmnSchemaDto {
  decision_name: string;
  hit_policy: string;
  inputs: { name: string; type: string }[];
  outputs: { name: string; type: string }[];
  rules: DmnRuleDto[];
}

