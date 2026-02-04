/**
 * Scope API
 *
 * API calls for session scope (CBU set) management.
 * Maps to backend routes at /api/session/:id/scope-graph and /api/cbu/:id/graph
 */

import { api } from "./client";

/**
 * CBU summary from graph nodes
 */
export interface CbuSummary {
  id: string;
  name: string;
  kind?: string;
  jurisdiction?: string;
  status?: string;
}

/**
 * Entity within a CBU
 */
export interface EntitySummary {
  id: string;
  name: string;
  entityType?: string;
  role?: string;
}

/**
 * Graph node from backend
 */
export interface GraphNode {
  id: string;
  node_type: string;
  label: string;
  kind?: string;
  entity_type?: string;
  data?: Record<string, unknown>;
}

/**
 * Graph edge from backend
 */
export interface GraphEdge {
  source: string;
  target: string;
  edge_type: string;
  label?: string;
}

/**
 * CBU Graph response from /api/cbu/:id/graph
 */
export interface CbuGraphResponse {
  cbu_id: string;
  cbu_name?: string;
  nodes: GraphNode[];
  edges: GraphEdge[];
}

/**
 * Session scope response from /api/session/:id/scope-graph
 */
export interface ScopeGraphResponse {
  graph?: {
    nodes: GraphNode[];
    edges: GraphEdge[];
  };
  cbu_ids: string[];
  cbu_count: number;
  affected_entity_ids: string[];
  error?: string;
}

/**
 * Extract CBU summaries from scope graph response
 */
function extractCbuSummaries(response: ScopeGraphResponse): CbuSummary[] {
  if (!response.graph?.nodes) {
    // If no graph, just return IDs
    return response.cbu_ids.map((id) => ({
      id,
      name: `CBU ${id.slice(0, 8)}`,
    }));
  }

  // Filter for CBU nodes and extract details
  const cbuNodes = response.graph.nodes.filter(
    (node) => node.node_type === "CBU" || node.node_type === "cbu",
  );

  return cbuNodes.map((node) => ({
    id: node.id,
    name:
      node.label || node.data?.name?.toString() || `CBU ${node.id.slice(0, 8)}`,
    kind: node.kind || node.data?.kind?.toString(),
    jurisdiction: node.data?.jurisdiction?.toString(),
    status: node.data?.status?.toString(),
  }));
}

/**
 * Extract entity summaries from CBU graph response
 */
function extractEntitySummaries(response: CbuGraphResponse): EntitySummary[] {
  return response.nodes
    .filter(
      (node) => node.node_type === "entity" || node.node_type === "Entity",
    )
    .map((node) => ({
      id: node.id,
      name:
        node.label ||
        node.data?.name?.toString() ||
        `Entity ${node.id.slice(0, 8)}`,
      entityType: node.entity_type || node.data?.entity_type?.toString(),
      role: node.data?.role?.toString(),
    }));
}

export const scopeApi = {
  /**
   * Get session scope (loaded CBUs)
   */
  async getScope(sessionId: string): Promise<{
    cbus: CbuSummary[];
    cbuCount: number;
    affectedEntityIds: string[];
    error?: string;
  }> {
    const response = await api.get<ScopeGraphResponse>(
      `/session/${sessionId}/scope-graph`,
    );

    return {
      cbus: extractCbuSummaries(response),
      cbuCount: response.cbu_count,
      affectedEntityIds: response.affected_entity_ids,
      error: response.error,
    };
  },

  /**
   * Get single CBU graph with entities
   */
  async getCbuGraph(cbuId: string): Promise<{
    cbuId: string;
    cbuName?: string;
    entities: EntitySummary[];
    nodeCount: number;
    edgeCount: number;
  }> {
    const response = await api.get<CbuGraphResponse>(`/cbu/${cbuId}/graph`);

    return {
      cbuId: response.cbu_id || cbuId,
      cbuName: response.cbu_name,
      entities: extractEntitySummaries(response),
      nodeCount: response.nodes?.length ?? 0,
      edgeCount: response.edges?.length ?? 0,
    };
  },
};

export default scopeApi;
