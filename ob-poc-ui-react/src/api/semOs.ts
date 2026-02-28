/**
 * Semantic OS API
 *
 * API calls for Semantic OS context and registry data.
 * Maps to backend route at /api/sem-os/* (see agent_routes.rs)
 */

import { api } from "./client";

export interface RegistryStats {
  [objectType: string]: number;
}

export interface ChangesetSummary {
  id: string;
  title: string;
  status: string;
  created_at: string;
  entry_count: number;
}

export interface SemOsContextResponse {
  registry_stats: RegistryStats;
  recent_changesets: ChangesetSummary[];
  agent_mode: string;
}

export const semOsApi = {
  /** Get Semantic OS context (registry stats, recent changesets, agent mode) */
  async getContext(): Promise<SemOsContextResponse> {
    return api.get<SemOsContextResponse>("/sem-os/context");
  },
};

export default semOsApi;
