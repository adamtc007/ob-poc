/**
 * REPL API Client
 *
 * API calls for the new REPL state machine architecture.
 * Maps to backend routes at /api/repl/* (see repl_routes.rs)
 *
 * This is the NEW unified API that replaces the legacy /api/session/* chat endpoints.
 * Key differences:
 * - Single /input endpoint for ALL user interactions (messages, selections, commands)
 * - Explicit state machine with clear transitions
 * - Session state includes full ledger for replay capability
 */

import { api } from "./client";

// ============================================================================
// Types matching Rust backend (repl/types.rs, repl_routes.rs)
// ============================================================================

/** REPL state machine states */
export type ReplState =
  | { type: "Idle" }
  | { type: "IntentMatching"; started_at: string }
  | { type: "Clarifying"; clarifying: ClarifyingState }
  | { type: "DslReady"; dsl: string; verb: string; can_auto_execute: boolean }
  | { type: "Executing"; dsl: string; started_at: string };

/** What we're waiting for user to clarify */
export type ClarifyingState =
  | {
      type: "VerbSelection";
      options: VerbOption[];
      original_input: string;
      margin: number;
    }
  | {
      type: "ScopeSelection";
      options: ScopeOption[];
      context: string;
    }
  | {
      type: "EntityResolution";
      unresolved_refs: UnresolvedRef[];
      partial_dsl: string;
    }
  | {
      type: "Confirmation";
      dsl: string;
      summary: string;
    }
  | {
      type: "IntentTier";
      tier_number: number;
      options: IntentTierOption[];
      prompt: string;
    }
  | {
      type: "ClientGroupSelection";
      options: ClientGroupOption[];
      prompt: string;
    };

export interface VerbOption {
  verb_fqn: string;
  description: string;
  score: number;
  example?: string;
}

export interface ScopeOption {
  id: string;
  name: string;
  description?: string;
}

export interface UnresolvedRef {
  ref_id: string;
  mention: string;
  entity_type?: string;
  candidates: EntityCandidate[];
}

export interface EntityCandidate {
  entity_id: string;
  name: string;
  score: number;
  entity_kind?: string;
}

export interface IntentTierOption {
  id: string;
  label: string;
  description: string;
  hint?: string;
  verb_count: number;
}

export interface ClientGroupOption {
  group_id: string;
  name: string;
  alias?: string;
}

/** Ledger entry - single source of truth for session history */
export interface LedgerEntry {
  id: string;
  timestamp: string;
  input: UserInput;
  intent_result?: IntentMatchResult;
  dsl?: string;
  execution_result?: LedgerExecutionResult;
  status: EntryStatus;
}

export type UserInput =
  | { type: "Message"; content: string }
  | {
      type: "VerbSelection";
      option_index: number;
      selected_verb: string;
      original_input: string;
    }
  | { type: "ScopeSelection"; option_id: string; option_name: string }
  | {
      type: "EntitySelection";
      ref_id: string;
      entity_id: string;
      entity_name: string;
    }
  | { type: "Confirmation"; confirmed: boolean }
  | { type: "IntentTierSelection"; tier: number; selected_id: string }
  | { type: "ClientGroupSelection"; group_id: string; group_name: string }
  | { type: "Command"; command: ReplCommand };

export type ReplCommand =
  | "Run"
  | "Undo"
  | "Redo"
  | "Clear"
  | "Cancel"
  | "Info"
  | "Help";

export type EntryStatus =
  | "Draft"
  | { Clarifying: string }
  | "Ready"
  | "Executing"
  | "Executed"
  | { Failed: string }
  | "Cancelled";

export interface IntentMatchResult {
  outcome: MatchOutcome;
  verb_candidates: VerbOption[];
  entity_mentions: EntityMention[];
  generated_dsl?: string;
}

export type MatchOutcome =
  | { type: "Matched"; verb: string; confidence: number }
  | { type: "Ambiguous"; margin: number }
  | { type: "NeedsScopeSelection" }
  | { type: "NeedsEntityResolution" }
  | { type: "NeedsClientGroup" }
  | { type: "NeedsIntentTier" }
  | { type: "NoMatch"; reason: string }
  | { type: "DirectDsl"; source: string };

export interface EntityMention {
  span: [number, number];
  text: string;
  entity_id?: string;
  entity_kind?: string;
  confidence?: number;
}

export interface LedgerExecutionResult {
  success: boolean;
  message: string;
  affected_cbu_ids?: string[];
  bindings?: Record<string, string>;
}

/** Derived state computed from ledger */
export interface DerivedState {
  cbu_ids: string[];
  bindings: Record<string, BoundEntity>;
  view_state: ViewState;
  messages: ChatMessage[];
}

export interface BoundEntity {
  entity_id: string;
  entity_name: string;
  entity_kind: string;
}

export interface ViewState {
  mode?: string;
  focus_entity_id?: string;
  filters?: string[];
}

export interface ChatMessage {
  role: "user" | "agent";
  content: string;
  timestamp: string;
}

// ============================================================================
// API Request/Response Types
// ============================================================================

/** Request to create a new session */
export interface CreateSessionResponse {
  session_id: string;
  state: ReplState;
}

/** Full session state response */
export interface SessionStateResponse {
  session_id: string;
  state: ReplState;
  client_group_id?: string;
  client_group_name?: string;
  derived: DerivedState;
  entry_count: number;
  recent_entries: LedgerEntry[];
}

/** Request to send input to session */
export type InputRequest =
  | { type: "message"; content: string }
  | {
      type: "verb_selection";
      option_index: number;
      selected_verb: string;
      original_input: string;
    }
  | { type: "scope_selection"; option_id: string; option_name: string }
  | {
      type: "entity_selection";
      ref_id: string;
      entity_id: string;
      entity_name: string;
    }
  | { type: "confirmation"; confirmed: boolean }
  | { type: "intent_tier_selection"; tier: number; selected_id: string }
  | { type: "client_group_selection"; group_id: string; group_name: string }
  | { type: "command"; command: string };

/** Response from the REPL orchestrator */
export interface ReplResponse {
  kind: ReplResponseKind;
  message: string;
  state: ReplState;
  dsl?: string;
  verb?: string;
  session_id: string;
}

export type ReplResponseKind =
  | "message"
  | "dsl_ready"
  | "dsl_executed"
  | "verb_disambiguation"
  | "scope_selection"
  | "entity_resolution"
  | "confirmation_required"
  | "intent_tier_selection"
  | "client_group_selection"
  | "no_match"
  | "error"
  | "info"
  | "help"
  | "cancelled";

// ============================================================================
// REPL API Client
// ============================================================================

export const replApi = {
  /**
   * Create a new REPL session
   * POST /api/repl/session
   */
  async createSession(): Promise<CreateSessionResponse> {
    return api.post<CreateSessionResponse>("/repl/session", {});
  },

  /**
   * Get full session state (for page reload recovery)
   * GET /api/repl/session/:id
   */
  async getSession(sessionId: string): Promise<SessionStateResponse> {
    return api.get<SessionStateResponse>(`/repl/session/${sessionId}`);
  },

  /**
   * Send any input to the REPL (unified endpoint)
   * POST /api/repl/session/:id/input
   *
   * This single endpoint handles ALL user interactions:
   * - Natural language messages
   * - Verb selection from disambiguation
   * - Scope/entity selection
   * - Confirmations
   * - Commands (run, undo, redo, clear, etc.)
   */
  async sendInput(
    sessionId: string,
    input: InputRequest
  ): Promise<ReplResponse> {
    return api.post<ReplResponse>(`/repl/session/${sessionId}/input`, input);
  },

  /**
   * Delete a session
   * DELETE /api/repl/session/:id
   */
  async deleteSession(sessionId: string): Promise<void> {
    await api.delete(`/repl/session/${sessionId}`);
  },

  // =========================================================================
  // Convenience methods for specific input types
  // =========================================================================

  /** Send a natural language message */
  async sendMessage(sessionId: string, content: string): Promise<ReplResponse> {
    return this.sendInput(sessionId, { type: "message", content });
  },

  /** Select a verb from disambiguation options */
  async selectVerb(
    sessionId: string,
    optionIndex: number,
    selectedVerb: string,
    originalInput: string
  ): Promise<ReplResponse> {
    return this.sendInput(sessionId, {
      type: "verb_selection",
      option_index: optionIndex,
      selected_verb: selectedVerb,
      original_input: originalInput,
    });
  },

  /** Select a scope option */
  async selectScope(
    sessionId: string,
    optionId: string,
    optionName: string
  ): Promise<ReplResponse> {
    return this.sendInput(sessionId, {
      type: "scope_selection",
      option_id: optionId,
      option_name: optionName,
    });
  },

  /** Resolve an entity reference */
  async selectEntity(
    sessionId: string,
    refId: string,
    entityId: string,
    entityName: string
  ): Promise<ReplResponse> {
    return this.sendInput(sessionId, {
      type: "entity_selection",
      ref_id: refId,
      entity_id: entityId,
      entity_name: entityName,
    });
  },

  /** Confirm or reject an action */
  async confirm(sessionId: string, confirmed: boolean): Promise<ReplResponse> {
    return this.sendInput(sessionId, { type: "confirmation", confirmed });
  },

  /** Select an intent tier option */
  async selectIntentTier(
    sessionId: string,
    tier: number,
    selectedId: string
  ): Promise<ReplResponse> {
    return this.sendInput(sessionId, {
      type: "intent_tier_selection",
      tier,
      selected_id: selectedId,
    });
  },

  /** Select a client group */
  async selectClientGroup(
    sessionId: string,
    groupId: string,
    groupName: string
  ): Promise<ReplResponse> {
    return this.sendInput(sessionId, {
      type: "client_group_selection",
      group_id: groupId,
      group_name: groupName,
    });
  },

  /** Send a REPL command */
  async sendCommand(
    sessionId: string,
    command: ReplCommand
  ): Promise<ReplResponse> {
    return this.sendInput(sessionId, {
      type: "command",
      command: command.toLowerCase(),
    });
  },

  /** Execute the staged DSL */
  async run(sessionId: string): Promise<ReplResponse> {
    return this.sendCommand(sessionId, "Run");
  },

  /** Undo the last action */
  async undo(sessionId: string): Promise<ReplResponse> {
    return this.sendCommand(sessionId, "Undo");
  },

  /** Redo the last undone action */
  async redo(sessionId: string): Promise<ReplResponse> {
    return this.sendCommand(sessionId, "Redo");
  },

  /** Clear the session */
  async clear(sessionId: string): Promise<ReplResponse> {
    return this.sendCommand(sessionId, "Clear");
  },

  /** Cancel current operation */
  async cancel(sessionId: string): Promise<ReplResponse> {
    return this.sendCommand(sessionId, "Cancel");
  },
};

export default replApi;
