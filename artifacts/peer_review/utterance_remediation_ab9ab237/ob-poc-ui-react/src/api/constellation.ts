/**
 * Constellation API
 *
 * Reads the server-side hydrated constellation payload used by the chat UI.
 */

import { api } from "./client";
import type {
  ConstellationContextRef,
  ProgressSummary,
  ResolvedConstellationContext,
  SubjectKind,
  WorkspaceKind,
  WorkspaceStateView,
} from "./replV2";

export interface BlockReason {
  message: string;
}

export interface BlockedVerb {
  verb: string;
  reasons: BlockReason[];
}

export interface RawOverlayRow {
  entity_id?: string | null;
  source_name: string;
  fields: Record<string, unknown>;
}

export interface HydratedGraphNode {
  entity_id: string;
  name?: string | null;
  entity_type?: string | null;
}

export interface HydratedGraphEdge {
  from_entity_id: string;
  to_entity_id: string;
  percentage?: number | null;
  ownership_type?: string | null;
  depth: number;
}

export interface ConstellationCaseSummary {
  case_id: string;
  status?: string | null;
  case_type?: string | null;
  opened_at?: string | null;
}

export interface HydratedSlot {
  name: string;
  path: string;
  slot_type: string;
  cardinality: string;
  entity_id?: string | null;
  record_id?: string | null;
  computed_state: string;
  effective_state: string;
  progress: number;
  blocking: boolean;
  warnings: string[];
  overlays: RawOverlayRow[];
  graph_node_count?: number | null;
  graph_edge_count?: number | null;
  graph_nodes: HydratedGraphNode[];
  graph_edges: HydratedGraphEdge[];
  available_verbs: string[];
  blocked_verbs: BlockedVerb[];
  children: HydratedSlot[];
}

export interface HydratedConstellation {
  constellation: string;
  description?: string | null;
  jurisdiction: string;
  map_revision: string;
  cbu_id: string;
  case_id?: string | null;
  slots: HydratedSlot[];
}

export interface OwnershipSummary {
  total_entities: number;
  total_edges: number;
  chain_complete: boolean;
}

export interface ConstellationSummary {
  total_slots: number;
  slots_filled: number;
  slots_empty_mandatory: number;
  slots_empty_optional: number;
  slots_placeholder: number;
  slots_complete: number;
  slots_in_progress: number;
  slots_blocked: number;
  blocking_slots: number;
  overall_progress: number;
  completion_pct: number;
  ownership_chain?: OwnershipSummary | null;
}

export const constellationApi = {
  async resolveContext(
    context: ConstellationContextRef,
  ): Promise<ResolvedConstellationContext> {
    return api.post<ResolvedConstellationContext>("/constellation/resolve", context);
  },

  async hydrateContext(params: {
    sessionId: string;
    clientGroupId: string;
    workspace: WorkspaceKind;
    constellationFamily?: string;
    constellationMap?: string;
    subjectKind?: SubjectKind;
    subjectId?: string;
  }): Promise<WorkspaceStateView> {
    return api.get<WorkspaceStateView>("/constellation/hydrate", {
      session_id: params.sessionId,
      client_group_id: params.clientGroupId,
      workspace: params.workspace,
      constellation_family: params.constellationFamily,
      constellation_map: params.constellationMap,
      subject_kind: params.subjectKind,
      subject_id: params.subjectId,
    });
  },

  async getWorkspaceSummary(params: {
    sessionId: string;
    clientGroupId: string;
    workspace: WorkspaceKind;
    constellationFamily?: string;
    constellationMap?: string;
    subjectKind?: SubjectKind;
    subjectId?: string;
  }): Promise<ProgressSummary> {
    return api.get<ProgressSummary>("/constellation/summary", {
      session_id: params.sessionId,
      client_group_id: params.clientGroupId,
      workspace: params.workspace,
      constellation_family: params.constellationFamily,
      constellation_map: params.constellationMap,
      subject_kind: params.subjectKind,
      subject_id: params.subjectId,
    });
  },

  async getConstellation(
    cbuId: string,
    params?: { caseId?: string; mapName?: string },
  ): Promise<HydratedConstellation> {
    return api.get<HydratedConstellation>(`/cbu/${cbuId}/constellation`, {
      case_id: params?.caseId,
      map_name: params?.mapName,
    });
  },

  async getSummary(
    cbuId: string,
    params?: { caseId?: string; mapName?: string },
  ): Promise<ConstellationSummary> {
    return api.get<ConstellationSummary>(
      `/cbu/${cbuId}/constellation/summary`,
      {
        case_id: params?.caseId,
        map_name: params?.mapName,
      },
    );
  },

  async getCases(cbuId: string): Promise<ConstellationCaseSummary[]> {
    return api.get<ConstellationCaseSummary[]>(`/cbu/${cbuId}/cases`);
  },
};

export default constellationApi;
