/**
 * Chat API
 *
 * API calls for agent chat sessions.
 * Maps to backend routes at /api/session/* (see agent_routes.rs)
 */

import { api } from "./client";
import type {
  ChatSession,
  ChatSessionSummary,
  SendMessageRequest,
  SendMessageResponse,
  DecisionReply,
  ChatMessage,
} from "../types/chat";

/**
 * Backend session response structure
 * The backend returns AgentSession which we map to our ChatSession type
 * Note: Backend uses session_id (create) or id (get) depending on endpoint
 */
interface BackendMessage {
  id?: string;
  role: "user" | "assistant" | "agent";
  content: string;
  timestamp?: string;
}

interface BackendSession {
  id?: string;
  session_id?: string;
  created_at: string;
  state?: string;
  welcome_message?: string;
  run_sheet?: {
    entries: Array<{
      id: string;
      status: string;
      source: string;
      timestamp: string;
    }>;
  };
  context?: {
    cbu_id?: string;
    cbu_ids?: string[];
    symbols?: Record<string, unknown>;
  };
  // Backend returns 'messages' array (not chat_history)
  messages?: BackendMessage[];
  // Legacy field name
  chat_history?: BackendMessage[];
}

/** Convert backend session to our chat session format */
function mapBackendSession(backend: BackendSession): ChatSession {
  // Backend uses session_id for create response, id for get response
  const sessionId = backend.session_id || backend.id || "";
  // Backend returns 'messages' (preferred) or 'chat_history' (legacy)
  const messages = backend.messages || backend.chat_history || [];
  return {
    id: sessionId,
    title: `Session ${sessionId.slice(0, 8)}`,
    created_at: backend.created_at,
    updated_at: backend.created_at,
    messages: messages.map((msg, idx) => ({
      id: msg.id || `${sessionId}-${idx}`,
      // Map 'agent' role to 'assistant' for UI consistency
      role: msg.role === "agent" ? "assistant" : msg.role,
      content: msg.content,
      timestamp: msg.timestamp || backend.created_at,
    })),
  };
}

export const chatApi = {
  /**
   * List all chat sessions
   * Note: Backend doesn't have a list endpoint, so we return empty for now
   * Sessions are created on-demand
   */
  listSessions(): Promise<ChatSessionSummary[]> {
    // Backend doesn't persist session list - return empty
    // In future, could store session IDs in localStorage
    const stored = localStorage.getItem("ob-poc-sessions");
    if (stored) {
      try {
        return Promise.resolve(JSON.parse(stored));
      } catch {
        return Promise.resolve([]);
      }
    }
    return Promise.resolve([]);
  },

  /** Get a chat session with messages */
  async getSession(id: string): Promise<ChatSession> {
    const backend = await api.get<BackendSession>(`/session/${id}`);
    return mapBackendSession(backend);
  },

  /** Create a new chat session */
  async createSession(title?: string): Promise<ChatSession> {
    // Backend returns CreateSessionResponse: { session_id, created_at, state, welcome_message, decision? }
    const backend = await api.post<
      BackendSession & {
        welcome_message?: string;
        decision?: {
          packet_id: string;
          kind: string;
          prompt: string;
          choices: Array<{
            id: string;
            label: string;
            description: string;
            is_escape?: boolean;
          }>;
          confirm_token?: string;
        };
      }
    >("/session", {});
    const session = mapBackendSession(backend);
    if (title) {
      session.title = title;
    }

    // If backend included a decision packet (e.g. client group selection),
    // create an initial assistant message with the decision attached
    if (backend.welcome_message && backend.decision) {
      const initialMessage: ChatMessage = {
        id: `${session.id}-welcome`,
        role: "assistant",
        content: backend.welcome_message,
        timestamp: session.created_at,
        decision_packet: {
          id: backend.decision.packet_id,
          kind: "clarification",
          payload: {
            question: backend.decision.prompt,
            options: backend.decision.choices.map((choice) => ({
              id: choice.id,
              label: choice.label,
              description: choice.description,
              value: choice.id,
            })),
          },
          confirm_token: backend.decision.confirm_token,
        },
      };
      // Only add if mapBackendSession didn't already produce messages
      if (session.messages.length === 0) {
        session.messages.push(initialMessage);
      }
    }

    // Store session ID locally for listing
    const stored = localStorage.getItem("ob-poc-sessions");
    const sessions: ChatSessionSummary[] = stored ? JSON.parse(stored) : [];
    sessions.unshift({
      id: session.id,
      title: session.title,
      created_at: session.created_at,
      updated_at: session.updated_at,
      message_count: 0,
    });
    localStorage.setItem(
      "ob-poc-sessions",
      JSON.stringify(sessions.slice(0, 50)),
    ); // Keep last 50

    return session;
  },

  /** Delete a chat session */
  async deleteSession(id: string): Promise<void> {
    // Always remove from local storage first — session list is localStorage-driven
    const stored = localStorage.getItem("ob-poc-sessions");
    if (stored) {
      const sessions: ChatSessionSummary[] = JSON.parse(stored);
      const filtered = sessions.filter((s) => s.id !== id);
      localStorage.setItem("ob-poc-sessions", JSON.stringify(filtered));
    }

    // Best-effort backend cleanup (idempotent — 204 even if not in memory)
    try {
      await api.delete(`/session/${id}`);
    } catch {
      // Backend delete is best-effort; localStorage is the source of truth for the session list
    }
  },

  /**
   * Send a message to a session (via /chat endpoint)
   * Backend: POST /api/session/:id/chat
   */
  async sendMessage(
    sessionId: string,
    request: SendMessageRequest,
  ): Promise<SendMessageResponse> {
    const response = await api.post<{
      message?: string;
      response?: string;
      dsl?: { source?: string } | string;
      generated_dsl?: string;
      session_state?: string;
      unresolved_refs?: unknown[];
      verb_disambiguation?: unknown;
      intent_tier?: unknown;
      clarification?: unknown;
      error?: string;
      // Backend DecisionPacket for client group/deal/verb clarification
      decision?: {
        packet_id: string;
        kind: string; // "ClarifyGroup" | "ClarifyDeal" | "ClarifyVerb" | "ClarifyScope"
        prompt: string;
        choices: Array<{
          id: string;
          label: string;
          description: string;
          is_escape?: boolean;
        }>;
        payload: unknown;
        confirm_token?: string;
      };
    }>(`/session/${sessionId}/chat`, { message: request.message });

    // Extract content from response - backend uses 'message' field
    let content = response.message || response.response || "";

    // If no message but we have DSL, show the DSL source
    if (!content && response.dsl) {
      if (typeof response.dsl === "string") {
        content = response.dsl;
      } else if (response.dsl.source) {
        content = `Generated DSL:\n\`\`\`lisp\n${response.dsl.source}\n\`\`\``;
      }
    }

    // Fallback to generated_dsl
    if (!content && response.generated_dsl) {
      content = response.generated_dsl;
    }

    // Show error if present
    if (response.error) {
      content = `Error: ${response.error}`;
    }

    // Map backend response to our format
    const assistantMessage: ChatMessage = {
      id: `${sessionId}-${Date.now()}`,
      role: "assistant",
      content: content || "No response from server.",
      timestamp: new Date().toISOString(),
    };

    // Check for decision packets
    if (response.verb_disambiguation) {
      const disambiguation = response.verb_disambiguation as {
        options?: Array<{ verb_fqn: string; description?: string }>;
      };
      assistantMessage.decision_packet = {
        id: `verb-${Date.now()}`,
        kind: "clarification",
        payload: {
          question: "Which operation did you mean?",
          options: (disambiguation.options || []).map((opt, idx) => ({
            id: `opt-${idx}`,
            label: opt.verb_fqn,
            description: opt.description,
            value: opt.verb_fqn,
          })),
        },
      };
    } else if (response.clarification) {
      const clarification = response.clarification as {
        question?: string;
        options?: Array<{ id: string; label: string; value: unknown }>;
      };
      assistantMessage.decision_packet = {
        id: `clarify-${Date.now()}`,
        kind: "clarification",
        payload: {
          question: clarification.question || "Please clarify:",
          options: (clarification.options || []).map((opt, idx) => ({
            id: opt.id || `opt-${idx}`,
            label: opt.label,
            value: opt.value,
          })),
        },
      };
    } else if (response.decision) {
      // Handle backend DecisionPacket for client group/deal selection
      // Maps backend DecisionPacket to frontend ClarificationPayload format
      assistantMessage.decision_packet = {
        id: response.decision.packet_id,
        kind: "clarification", // All clarify kinds map to 'clarification'
        payload: {
          question: response.decision.prompt,
          options: response.decision.choices.map((choice) => ({
            id: choice.id,
            label: choice.label,
            description: choice.description,
            value: choice.id, // Use ID as value for selection
          })),
        },
        confirm_token: response.decision.confirm_token,
      };
    }

    // Update local storage message count
    const stored = localStorage.getItem("ob-poc-sessions");
    if (stored) {
      const sessions: ChatSessionSummary[] = JSON.parse(stored);
      const session = sessions.find((s) => s.id === sessionId);
      if (session) {
        session.message_count = (session.message_count || 0) + 2;
        session.updated_at = new Date().toISOString();
        session.last_message_preview = request.message.slice(0, 100);
        localStorage.setItem("ob-poc-sessions", JSON.stringify(sessions));
      }
    }

    return {
      message: assistantMessage,
      session: await chatApi.getSession(sessionId),
    };
  },

  /** Reply to a decision packet */
  async replyToDecision(
    sessionId: string,
    reply: DecisionReply,
  ): Promise<ChatMessage> {
    // Map frontend DecisionReply to backend DecisionReplyRequest format
    // Backend expects: { packet_id, reply: UserReply } where UserReply is a tagged enum
    let userReply: unknown;
    if (reply.selected_option !== undefined) {
      // selected_option is the choice ID (e.g. "1", "2") — convert to 0-indexed
      const index = parseInt(reply.selected_option, 10);
      userReply = { Select: { index: isNaN(index) ? 0 : index - 1 } };
    } else if (reply.freeform_response !== undefined) {
      userReply = { TypeExact: { text: reply.freeform_response } };
    } else if (reply.confirmed !== undefined) {
      userReply = reply.confirmed
        ? { Confirm: { token: reply.confirm_token || null } }
        : "Cancel";
    } else {
      userReply = "Cancel";
    }

    const response = await api.post<{
      message?: string;
      response?: string;
      next_packet?: unknown;
    }>(`/session/${sessionId}/decision/reply`, {
      packet_id: reply.packet_id,
      reply: userReply,
    });
    return {
      id: `${sessionId}-${Date.now()}`,
      role: "assistant",
      content: response.message || response.response || "Decision recorded.",
      timestamp: new Date().toISOString(),
    };
  },

  /** Get WebSocket URL for streaming */
  getStreamUrl(sessionId: string): string {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    return `${protocol}//${window.location.host}/api/session/${sessionId}/stream`;
  },
};

export default chatApi;
