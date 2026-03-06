import { ApiError } from "./client";
import type { ChatSessionSummary } from "../types/chat";

export const CHAT_SESSIONS_STORAGE_KEY = "ob-poc-sessions";
export const SEMOS_SESSIONS_STORAGE_KEY = "ob-poc-semos-sessions";

function safeRead(key: string): ChatSessionSummary[] {
  const raw = localStorage.getItem(key);
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw) as ChatSessionSummary[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function safeWrite(key: string, sessions: ChatSessionSummary[]) {
  localStorage.setItem(key, JSON.stringify(sessions));
}

/**
 * Remove a session ID from both chat and Semantic OS local session lists.
 */
export function pruneSessionIdFromStorage(sessionId: string) {
  for (const key of [CHAT_SESSIONS_STORAGE_KEY, SEMOS_SESSIONS_STORAGE_KEY]) {
    const next = safeRead(key).filter((s) => s.id !== sessionId);
    safeWrite(key, next);
  }
}

/**
 * Return true when an API error indicates the requested session no longer exists.
 */
export function isSessionMissingError(error: unknown): boolean {
  if (!(error instanceof ApiError) || error.status !== 404) {
    return false;
  }

  const body = error.body;
  if (typeof body === "string") {
    return body.toLowerCase().includes("session")
      && body.toLowerCase().includes("not found");
  }
  if (body && typeof body === "object") {
    const maybeError = (body as { error?: unknown }).error;
    if (typeof maybeError === "string") {
      return maybeError.toLowerCase().includes("session")
        && maybeError.toLowerCase().includes("not found");
    }
  }
  return false;
}
