/**
 * Projection API
 *
 * API calls for inspector projections.
 */

import { api } from './client';
import type {
  InspectorProjection,
  ProjectionListItem,
  ProjectionPolicy,
  ProjectionNode,
  ChildrenResponse,
  ValidationResult,
} from '../types/projection';

export interface GenerateProjectionRequest {
  snapshot_id: string;
  policy: ProjectionPolicy;
}

export const projectionsApi = {
  /** List all projections */
  list(): Promise<ProjectionListItem[]> {
    return api.get<ProjectionListItem[]>('/projections');
  },

  /** Get a full projection by ID */
  get(id: string): Promise<InspectorProjection> {
    return api.get<InspectorProjection>(`/projections/${id}`);
  },

  /** Get a single node from a projection */
  getNode(projectionId: string, nodeId: string): Promise<ProjectionNode> {
    return api.get<ProjectionNode>(`/projections/${projectionId}/nodes/${nodeId}`);
  },

  /** Get paginated children of a node */
  getNodeChildren(
    projectionId: string,
    nodeId: string,
    params?: { offset?: number; limit?: number }
  ): Promise<ChildrenResponse> {
    return api.get<ChildrenResponse>(
      `/projections/${projectionId}/nodes/${nodeId}/children`,
      params
    );
  },

  /** Generate a new projection from a snapshot */
  generate(request: GenerateProjectionRequest): Promise<InspectorProjection> {
    return api.post<InspectorProjection>('/projections/generate', request);
  },

  /** Validate a projection */
  validate(projection: InspectorProjection): Promise<ValidationResult> {
    return api.post<ValidationResult>('/projections/validate', projection);
  },
};

export default projectionsApi;
