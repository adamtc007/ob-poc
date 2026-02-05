/**
 * Deal API
 *
 * API calls for deal taxonomy management.
 * Maps to backend routes at /api/deal/*
 */

import { api } from "./client";
import type {
  DealSummary,
  DealGraphResponse,
  DealProductSummary,
  RateCardSummary,
  RateCardLineSummary,
  DealParticipantSummary,
  DealContractSummary,
  OnboardingRequestSummary,
  SessionDealContext,
  DealFilters,
  DealViewMode,
} from "../types/deal";

/** Deal list response from backend */
interface DealListApiResponse {
  deals: DealSummary[];
  total: number;
  limit: number;
  offset: number;
}

export const dealApi = {
  /**
   * List all deals with optional filters
   */
  async listDeals(filters?: DealFilters): Promise<DealListApiResponse> {
    return api.get<DealListApiResponse>("/deals", {
      search: filters?.search,
      status: filters?.status,
      client_group_id: filters?.client_group_id,
      limit: filters?.limit,
      offset: filters?.offset,
    });
  },

  /**
   * Get deal summary by ID
   */
  async getDeal(dealId: string): Promise<DealSummary> {
    return api.get<DealSummary>(`/deal/${dealId}`);
  },

  /**
   * Get full deal graph with all related data
   */
  async getDealGraph(
    dealId: string,
    viewMode: DealViewMode = "COMMERCIAL",
  ): Promise<DealGraphResponse> {
    return api.get<DealGraphResponse>(`/deal/${dealId}/graph`, {
      view_mode: viewMode,
    });
  },

  /**
   * Get products for a deal
   */
  async getDealProducts(dealId: string): Promise<DealProductSummary[]> {
    return api.get<DealProductSummary[]>(`/deal/${dealId}/products`);
  },

  /**
   * Get rate cards for a deal
   */
  async getDealRateCards(dealId: string): Promise<RateCardSummary[]> {
    return api.get<RateCardSummary[]>(`/deal/${dealId}/rate-cards`);
  },

  /**
   * Get rate card lines
   */
  async getRateCardLines(rateCardId: string): Promise<RateCardLineSummary[]> {
    return api.get<RateCardLineSummary[]>(`/deal/rate-card/${rateCardId}/lines`);
  },

  /**
   * Get rate card supersession history
   */
  async getRateCardHistory(rateCardId: string): Promise<RateCardSummary[]> {
    return api.get<RateCardSummary[]>(`/deal/rate-card/${rateCardId}/history`);
  },

  /**
   * Get participants for a deal
   */
  async getDealParticipants(
    dealId: string,
  ): Promise<DealParticipantSummary[]> {
    return api.get<DealParticipantSummary[]>(`/deal/${dealId}/participants`);
  },

  /**
   * Get contracts for a deal
   */
  async getDealContracts(dealId: string): Promise<DealContractSummary[]> {
    return api.get<DealContractSummary[]>(`/deal/${dealId}/contracts`);
  },

  /**
   * Get onboarding requests for a deal
   */
  async getDealOnboardingRequests(
    dealId: string,
  ): Promise<OnboardingRequestSummary[]> {
    return api.get<OnboardingRequestSummary[]>(
      `/deal/${dealId}/onboarding-requests`,
    );
  },

  /**
   * Get current deal context from session
   */
  async getSessionDealContext(sessionId: string): Promise<SessionDealContext> {
    return api.get<SessionDealContext>(`/session/${sessionId}/deal-context`);
  },
};

export default dealApi;
