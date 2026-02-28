/**
 * TanStack Query Configuration
 */

import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5, // 5 minutes
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

/** Query keys for type-safe cache management */
export const queryKeys = {
  // Projections
  projections: {
    all: ["projections"] as const,
    list: () => [...queryKeys.projections.all, "list"] as const,
    detail: (id: string) =>
      [...queryKeys.projections.all, "detail", id] as const,
    node: (projectionId: string, nodeId: string) =>
      [...queryKeys.projections.all, "node", projectionId, nodeId] as const,
    children: (projectionId: string, nodeId: string) =>
      [...queryKeys.projections.all, "children", projectionId, nodeId] as const,
  },

  // Chat
  chat: {
    all: ["chat"] as const,
    sessions: () => [...queryKeys.chat.all, "sessions"] as const,
    session: (id: string) => [...queryKeys.chat.all, "session", id] as const,
  },

  // Entities
  entities: {
    all: ["entities"] as const,
    detail: (id: string) => [...queryKeys.entities.all, "detail", id] as const,
    search: (query: string) =>
      [...queryKeys.entities.all, "search", query] as const,
  },

  // Session Scope
  scope: (sessionId: string) => ["scope", sessionId] as const,

  // Semantic OS
  semOs: {
    all: ["semOs"] as const,
    sessions: () => [...queryKeys.semOs.all, "sessions"] as const,
    session: (id: string) => [...queryKeys.semOs.all, "session", id] as const,
    context: () => [...queryKeys.semOs.all, "context"] as const,
  },

  // Deals
  deals: {
    all: ["deals"] as const,
    list: (filters?: Record<string, unknown>) =>
      [...queryKeys.deals.all, "list", filters] as const,
    detail: (id: string) => [...queryKeys.deals.all, "detail", id] as const,
    graph: (id: string, viewMode: string) =>
      [...queryKeys.deals.all, "graph", id, viewMode] as const,
    products: (dealId: string) =>
      [...queryKeys.deals.all, "products", dealId] as const,
    rateCards: (dealId: string) =>
      [...queryKeys.deals.all, "rateCards", dealId] as const,
    rateCardLines: (rateCardId: string) =>
      [...queryKeys.deals.all, "rateCardLines", rateCardId] as const,
    rateCardHistory: (rateCardId: string) =>
      [...queryKeys.deals.all, "rateCardHistory", rateCardId] as const,
    participants: (dealId: string) =>
      [...queryKeys.deals.all, "participants", dealId] as const,
    contracts: (dealId: string) =>
      [...queryKeys.deals.all, "contracts", dealId] as const,
    onboardingRequests: (dealId: string) =>
      [...queryKeys.deals.all, "onboardingRequests", dealId] as const,
    sessionContext: (sessionId: string) =>
      [...queryKeys.deals.all, "sessionContext", sessionId] as const,
  },
};

export default queryClient;
