/**
 * Chat API
 *
 * API calls for agent chat sessions.
 * Maps to backend routes at /api/session/* (see agent_routes.rs)
 */

import { api } from "./client";
import {
  CHAT_SESSIONS_STORAGE_KEY,
  isSessionMissingError,
  pruneSessionIdFromStorage,
} from "./sessionStorage";
import type {
  ChatSession,
  ChatSessionSummary,
  SendMessageRequest,
  SendMessageResponse,
  DecisionReply,
  ChatMessage,
  CoderProposal,
  DiscoverySelection,
  DiscoveryBootstrap,
  SageExplain,
  VerbProfile,
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
  sage_explain?: SageExplain;
  coder_proposal?: CoderProposal;
  discovery_bootstrap?: DiscoveryBootstrap;
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
      sage_explain: msg.sage_explain,
      coder_proposal: msg.coder_proposal,
      discovery_bootstrap: msg.discovery_bootstrap,
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
    const stored = localStorage.getItem(CHAT_SESSIONS_STORAGE_KEY);
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
    try {
      const backend = await api.get<BackendSession>(`/session/${id}`);
      return mapBackendSession(backend);
    } catch (error) {
      if (isSessionMissingError(error)) {
        pruneSessionIdFromStorage(id);
      }
      throw error;
    }
  },

  /** Create a new chat session */
  async createSession(
    title?: string,
    workflowFocus?: string,
  ): Promise<ChatSession> {
    // Backend returns CreateSessionResponse: { session_id, created_at, state, welcome_message, decision? }
    const body: Record<string, unknown> = {};
    if (workflowFocus) {
      body.workflow_focus = workflowFocus;
    }
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
    >("/session", body);
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
    const stored = localStorage.getItem(CHAT_SESSIONS_STORAGE_KEY);
    const sessions: ChatSessionSummary[] = stored ? JSON.parse(stored) : [];
    sessions.unshift({
      id: session.id,
      title: session.title,
      created_at: session.created_at,
      updated_at: session.updated_at,
      message_count: 0,
    });
    localStorage.setItem(
      CHAT_SESSIONS_STORAGE_KEY,
      JSON.stringify(sessions.slice(0, 50)),
    ); // Keep last 50

    return session;
  },

  /** Delete a chat session */
  async deleteSession(id: string): Promise<void> {
    // Always remove from local storage first — session list is localStorage-driven
    const stored = localStorage.getItem(CHAT_SESSIONS_STORAGE_KEY);
    if (stored) {
      const sessions: ChatSessionSummary[] = JSON.parse(stored);
      const filtered = sessions.filter((s) => s.id !== id);
      localStorage.setItem(CHAT_SESSIONS_STORAGE_KEY, JSON.stringify(filtered));
    }

    // Best-effort backend cleanup (idempotent — 204 even if not in memory)
    try {
      await api.delete(`/session/${id}`);
    } catch {
      // Backend delete is best-effort; localStorage is the source of truth for the session list
    }
  },

  /**
   * Send a message to a session (via unified /input endpoint)
   * Backend: POST /api/session/:id/input with { kind: "utterance", ... }
   */
  async sendMessage(
    sessionId: string,
    request: SendMessageRequest,
  ): Promise<SendMessageResponse> {
    let envelope: ChatInputEnvelope;
    try {
      envelope = await api.post<ChatInputEnvelope>(
        `/session/${sessionId}/input`,
        {
          kind: "utterance",
          message: request.message,
        },
      );
    } catch (error) {
      if (isSessionMissingError(error)) {
        pruneSessionIdFromStorage(sessionId);
      }
      throw error;
    }
    const response = envelope.response;
    const assistantMessage = buildAssistantMessage(sessionId, response);

    // Update local storage message count
    const stored = localStorage.getItem(CHAT_SESSIONS_STORAGE_KEY);
    if (stored) {
      const sessions: ChatSessionSummary[] = JSON.parse(stored);
      const session = sessions.find((s) => s.id === sessionId);
      if (session) {
        session.message_count = (session.message_count || 0) + 2;
        session.updated_at = new Date().toISOString();
        session.last_message_preview = request.message.slice(0, 100);
        localStorage.setItem(
          CHAT_SESSIONS_STORAGE_KEY,
          JSON.stringify(sessions),
        );
      }
    }

    // Re-fetch session for SemOs page (best-effort — don't let it break message delivery)
    let session: ChatSession | undefined;
    try {
      session = await chatApi.getSession(sessionId);
    } catch (e) {
      console.warn("[chat] Failed to re-fetch session after chat:", e);
    }

    return {
      message: assistantMessage,
      session: session || {
        id: sessionId,
        title: "",
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        messages: [],
      },
      available_verbs: response.available_verbs,
      surface_fingerprint: response.surface_fingerprint,
    };
  },

  async sendDiscoverySelection(
    sessionId: string,
    selection: DiscoverySelection,
  ): Promise<SendMessageResponse> {
    let envelope: ChatInputEnvelope;
    try {
      envelope = await api.post<ChatInputEnvelope>(
        `/session/${sessionId}/input`,
        {
          kind: "discovery_selection",
          selection,
        },
      );
    } catch (error) {
      if (isSessionMissingError(error)) {
        pruneSessionIdFromStorage(sessionId);
      }
      throw error;
    }
    const response = envelope.response;
    const assistantMessage = buildAssistantMessage(sessionId, response);

    let session: ChatSession | undefined;
    try {
      session = await chatApi.getSession(sessionId);
    } catch (e) {
      console.warn(
        "[chat] Failed to re-fetch session after discovery selection:",
        e,
      );
    }

    return {
      message: assistantMessage,
      session: session || {
        id: sessionId,
        title: "",
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        messages: [],
      },
      available_verbs: response.available_verbs,
      surface_fingerprint: response.surface_fingerprint,
    };
  },

  /** Reply to a decision packet */
  async replyToDecision(
    sessionId: string,
    reply: DecisionReply,
  ): Promise<{
    message: ChatMessage;
    available_verbs?: VerbProfile[];
    surface_fingerprint?: string;
  }> {
    // Map frontend DecisionReply to backend UserReply (tagged with "action").
    let userReply: unknown;
    if (reply.selected_option !== undefined) {
      // selected_option is the choice ID (e.g. "1", "2") — convert to 0-indexed.
      const index = parseInt(reply.selected_option, 10);
      userReply = { action: "select", index: isNaN(index) ? 0 : index - 1 };
    } else if (reply.freeform_response !== undefined) {
      userReply = { action: "type_exact", text: reply.freeform_response };
    } else if (reply.confirmed !== undefined) {
      userReply = reply.confirmed
        ? { action: "confirm", token: reply.confirm_token || null }
        : { action: "cancel" };
    } else {
      userReply = { action: "cancel" };
    }

    let envelope: {
      kind: "decision";
      response: {
        message?: string;
        response?: string;
        next_packet?: unknown;
        sage_explain?: SageExplain;
        coder_proposal?: CoderProposal;
        available_verbs?: VerbProfile[];
        surface_fingerprint?: string;
      };
    };
    try {
      envelope = await api.post<{
        kind: "decision";
        response: {
          message?: string;
          response?: string;
          next_packet?: unknown;
          sage_explain?: SageExplain;
          coder_proposal?: CoderProposal;
          available_verbs?: VerbProfile[];
          surface_fingerprint?: string;
        };
      }>(`/session/${sessionId}/input`, {
        kind: "decision_reply",
        packet_id: reply.packet_id,
        reply: userReply,
      });
    } catch (error) {
      if (isSessionMissingError(error)) {
        pruneSessionIdFromStorage(sessionId);
      }
      throw error;
    }
    const response = envelope.response;

    const verbs: VerbProfile[] = (response.available_verbs || []).map((v) => ({
      fqn: v.fqn,
      domain: v.domain,
      description: v.description,
      sexpr: `(${v.fqn})`,
      args: v.args || [],
      preconditions_met: v.preconditions_met ?? true,
      governance_tier: v.governance_tier || "operational",
    }));

    return {
      message: {
        id: `${sessionId}-${Date.now()}`,
        role: "assistant" as const,
        content: response.message || response.response || "Decision recorded.",
        timestamp: new Date().toISOString(),
        sage_explain: response.sage_explain,
        coder_proposal: response.coder_proposal,
      },
      available_verbs: verbs.length > 0 ? verbs : undefined,
      surface_fingerprint: response.surface_fingerprint,
    };
  },

  /** Get verb surface for a session */
  async getVerbSurface(sessionId: string): Promise<{
    verbs: VerbProfile[];
    surface_fingerprint?: string;
    totalRegistry?: number;
  }> {
    let response: {
      verbs?: Array<{
        fqn: string;
        domain: string;
        action?: string;
        description: string;
        governance_tier?: string;
        preconditions_met?: boolean;
      }>;
      filter_summary?: {
        total_registry: number;
        final_count: number;
      };
      surface_fingerprint?: string;
    };
    try {
      response = await api.get<{
        verbs?: Array<{
          fqn: string;
          domain: string;
          action?: string;
          description: string;
          governance_tier?: string;
          preconditions_met?: boolean;
        }>;
        filter_summary?: {
          total_registry: number;
          final_count: number;
        };
        surface_fingerprint?: string;
      }>(`/session/${sessionId}/verb-surface`);
    } catch (error) {
      if (isSessionMissingError(error)) {
        pruneSessionIdFromStorage(sessionId);
      }
      throw error;
    }

    const verbs: VerbProfile[] = (response.verbs || []).map((v) => ({
      fqn: v.fqn,
      domain: v.domain,
      description: v.description,
      sexpr: `(${v.fqn})`,
      args: [],
      preconditions_met: v.preconditions_met ?? true,
      governance_tier: v.governance_tier || "operational",
    }));

    return {
      verbs,
      surface_fingerprint: response.surface_fingerprint,
      totalRegistry: response.filter_summary?.total_registry,
    };
  },

  /** Get WebSocket URL for streaming */
  getStreamUrl(sessionId: string): string {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    return `${protocol}//${window.location.host}/api/session/${sessionId}/stream`;
  },
};

interface ChatInputEnvelope {
  kind: "chat";
  response: {
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
    sage_explain?: SageExplain;
    coder_proposal?: CoderProposal;
    discovery_bootstrap?: DiscoveryBootstrap;
    available_verbs?: VerbProfile[];
    surface_fingerprint?: string;
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
      payload: unknown;
      confirm_token?: string;
    };
  };
}

function buildAssistantMessage(
  sessionId: string,
  response: ChatInputEnvelope["response"],
): ChatMessage {
  let content = response.message || response.response || "";

  if (!content && response.dsl) {
    if (typeof response.dsl === "string") {
      content = response.dsl;
    } else if (response.dsl.source) {
      content = `Generated DSL:\n\`\`\`lisp\n${response.dsl.source}\n\`\`\``;
    }
  }

  if (!content && response.generated_dsl) {
    content = response.generated_dsl;
  }

  if (response.error) {
    content = `Error: ${response.error}`;
  }

  const assistantMessage: ChatMessage = {
    id: `${sessionId}-${Date.now()}`,
    role: "assistant",
    content: content || "No response from server.",
    timestamp: new Date().toISOString(),
    sage_explain: response.sage_explain,
    coder_proposal: response.coder_proposal,
    discovery_bootstrap: response.discovery_bootstrap,
  };

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
    assistantMessage.decision_packet = {
      id: response.decision.packet_id,
      kind: "clarification",
      payload: {
        question: response.decision.prompt,
        options: response.decision.choices.map((choice) => ({
          id: choice.id,
          label: choice.label,
          description: choice.description,
          value: choice.id,
        })),
      },
      confirm_token: response.decision.confirm_token,
    };
  }

  return assistantMessage;
}

export default chatApi;
