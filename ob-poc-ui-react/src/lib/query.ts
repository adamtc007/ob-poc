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
};

export default queryClient;
