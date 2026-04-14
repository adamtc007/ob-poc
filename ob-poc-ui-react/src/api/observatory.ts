/**
 * Observatory API
 *
 * API calls for the Observatory visualization layer.
 * Maps to backend routes at /api/observatory/* (see observatory_routes.rs)
 */

import { api } from "./client";
import type {
  OrientationContract,
  GraphSceneModel,
  HealthMetrics,
} from "../types/observatory";

export const observatoryApi = {
  /** Fetch current orientation contract for a session */
  async getOrientation(sessionId: string): Promise<OrientationContract> {
    return api.get<OrientationContract>(
      `/observatory/session/${sessionId}/orientation`,
    );
  },

  /** Fetch the ShowPacket (viewport data) for a session */
  async getShowPacket(sessionId: string): Promise<unknown> {
    return api.get<unknown>(`/observatory/session/${sessionId}/show-packet`);
  },

  /** Fetch the graph scene model for constellation rendering */
  async getGraphScene(sessionId: string): Promise<GraphSceneModel> {
    return api.get<GraphSceneModel>(
      `/observatory/session/${sessionId}/graph-scene`,
    );
  },

  /** Fetch the canonical session stack DAG scene */
  async getSessionStackGraph(sessionId: string): Promise<GraphSceneModel> {
    return api.get<GraphSceneModel>(
      `/observatory/session/${sessionId}/session-stack-graph`,
    );
  },

  /** Fetch navigation history with cursor (breadcrumbs) */
  async getNavigationHistory(
    sessionId: string,
  ): Promise<{ entries: OrientationContract[]; cursor: number }> {
    return api.get<{ entries: OrientationContract[]; cursor: number }>(
      `/observatory/session/${sessionId}/navigation-history`,
    );
  },

  /** Fetch observatory health metrics */
  async getHealth(): Promise<HealthMetrics> {
    return api.get<HealthMetrics>(`/observatory/health`);
  },

  /** Execute a navigation verb and return updated orientation + graph scene */
  async navigate(
    sessionId: string,
    verb: string,
    args: Record<string, unknown>,
  ): Promise<{ orientation: OrientationContract; graph_scene: GraphSceneModel }> {
    return api.post<{
      orientation: OrientationContract;
      graph_scene: GraphSceneModel;
    }>(`/observatory/session/${sessionId}/navigate`, { verb, args });
  },

  /** Fetch a diagram by type (e.g. mermaid) */
  async getDiagram(
    sessionId: string,
    type: string,
  ): Promise<{ diagram_type: string; mermaid: string }> {
    return api.get<{ diagram_type: string; mermaid: string }>(
      `/observatory/session/${sessionId}/diagrams/${type}`,
    );
  },
};

export default observatoryApi;
