/**
 * Catalogue Workspace API — Tranche 3 Phase 3.D scaffold (2026-04-27).
 *
 * v1.2 §8 Tranche 3 — governed authorship mechanism. Read-only client
 * for the Catalogue workspace REST scaffold (rust/src/api/catalogue_routes.rs).
 *
 * Used by the Observatory Catalogue panel to render:
 *   - Pending proposal list (DRAFT + STAGED).
 *   - Single proposal detail (proposed declaration JSON for diff render).
 *   - Live tier-distribution heatmap (Phase 2.G.2 data, post-Stage-4 source).
 *
 * Full canvas integration (egui WASM Phase 8 with diff preview + ABAC
 * two-eye visualization + live heatmap canvas component) is follow-on.
 * This client + the React panel below give the Observatory canvas its
 * data API in the meantime.
 */

import { api } from "./client";

export type ProposalStatus =
  | "DRAFT"
  | "STAGED"
  | "COMMITTED"
  | "ROLLED_BACK"
  | "REJECTED";

export type ProposalListFilter =
  | "pending"
  | "committed"
  | "rolled_back"
  | "all";

export interface ProposalSummary {
  proposal_id: string;
  verb_fqn: string;
  status: ProposalStatus;
  proposed_by: string;
  created_at: string;
  committed_by?: string;
  rolled_back_by?: string;
}

export interface ProposalDetail {
  proposal_id: string;
  verb_fqn: string;
  status: ProposalStatus;
  proposed_by: string;
  proposed_declaration: unknown; // full verb YAML fragment, JSON-serialised
  rationale?: string;
  created_at: string;
  staged_at?: string;
  committed_by?: string;
  committed_at?: string;
  rolled_back_by?: string;
  rolled_back_at?: string;
  rolled_back_reason?: string;
}

export interface TierDistribution {
  by_tier: Record<string, number>;
  by_domain_tier: Record<string, Record<string, number>>;
  total_verbs: number;
  three_axis_declared: number;
}

export const catalogueApi = {
  /** List proposals filtered by status. Default: pending (DRAFT + STAGED). */
  async listProposals(
    status: ProposalListFilter = "pending",
  ): Promise<ProposalSummary[]> {
    return api.get<ProposalSummary[]>(`/catalogue/proposals?status=${status}`);
  },

  /** Fetch single proposal detail (incl. full proposed declaration JSON). */
  async getProposal(proposalId: string): Promise<ProposalDetail> {
    return api.get<ProposalDetail>(`/catalogue/proposals/${proposalId}`);
  },

  /** Live tier-distribution heatmap data (Phase 2.G.2). */
  async getTierDistribution(): Promise<TierDistribution> {
    return api.get<TierDistribution>("/catalogue/tier-distribution");
  },
};
