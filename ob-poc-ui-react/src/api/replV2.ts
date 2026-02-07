/**
 * REPL V2 API Client — Pack-Guided Runbook Pipeline
 *
 * API calls for the V2 REPL orchestrator (sentence-first, pack-guided).
 * Maps to backend routes at /api/repl/v2/* (see repl_routes_v2.rs).
 *
 * Coexists with the V1 REPL client (repl.ts) which uses /api/repl/*.
 * V2 adds:
 * - Pack-guided journeys (scope → pack → verb → sentence → runbook)
 * - Sentence-first responses (human-readable playback)
 * - IntentService facade with clarification via sentences.clarify templates
 * - Runbook editing with chapter grouping
 */

import { api } from "./client";

// ============================================================================
// Types matching Rust backend (types_v2.rs, response_v2.rs, repl_routes_v2.rs)
// ============================================================================

/** V2 REPL 7-state machine */
export type ReplStateV2 =
  | { state: "scope_gate"; pending_input?: string }
  | { state: "journey_selection"; candidates?: PackCandidate[] }
  | {
      state: "in_pack";
      pack_id: string;
      required_slots_remaining: string[];
      last_proposal_id?: string;
    }
  | {
      state: "clarifying";
      question: string;
      candidates: VerbCandidate[];
      original_input: string;
    }
  | {
      state: "sentence_playback";
      sentence: string;
      verb: string;
      dsl: string;
      args: Record<string, string>;
    }
  | { state: "runbook_editing" }
  | {
      state: "executing";
      runbook_id: string;
      progress: ExecutionProgress;
    };

export interface PackCandidate {
  pack_id: string;
  pack_name: string;
  description: string;
  score: number;
}

export interface VerbCandidate {
  verb_fqn: string;
  description: string;
  score: number;
}

export interface ExecutionProgress {
  total_steps: number;
  completed_steps: number;
  failed_steps: number;
  current_step?: string;
}

/** V2 response from the orchestrator */
export interface ReplResponseV2 {
  state: ReplStateV2;
  kind: ReplResponseKindV2;
  message: string;
  runbook_summary?: string;
  step_count: number;
}

/** Response kind — determines UI rendering */
export type ReplResponseKindV2 =
  | { kind: "scope_required"; prompt: string }
  | { kind: "journey_options"; packs: PackCandidate[] }
  | { kind: "question"; field: string; prompt: string; answer_kind: string }
  | {
      kind: "sentence_playback";
      sentence: string;
      verb: string;
      step_sequence: number;
    }
  | {
      kind: "runbook_summary";
      chapters: ChapterView[];
      summary: string;
    }
  | {
      kind: "clarification";
      question: string;
      options: VerbCandidate[];
    }
  | { kind: "executed"; results: StepResult[] }
  | {
      kind: "step_proposals";
      proposals: StepProposal[];
      template_fast_path: boolean;
      proposal_hash: string;
    }
  | { kind: "error"; error: string; recoverable: boolean };

export interface ChapterView {
  chapter: string;
  steps: [number, string][]; // [sequence, sentence]
}

export interface StepResult {
  entry_id: string;
  sequence: number;
  sentence: string;
  success: boolean;
  message?: string;
  result?: unknown;
}

/** Evidence explaining why a proposal was generated (Phase 3) */
export interface ProposalEvidence {
  source: { kind: string; template_id?: string };
  confidence: number;
  rationale: string;
  missing_required_args: number;
  template_fit_score?: number;
  verb_search_score?: number;
}

/** A single proposed step with evidence (Phase 3) */
export interface StepProposal {
  id: string;
  verb: string;
  sentence: string;
  dsl: string;
  args: Record<string, string>;
  evidence: ProposalEvidence;
  confirm_policy: string;
}

// ============================================================================
// API Request Types
// ============================================================================

/** Request to send input to a V2 session */
export type InputRequestV2 =
  | { type: "message"; content: string }
  | { type: "confirm" }
  | { type: "reject" }
  | { type: "edit"; step_id: string; field: string; value: string }
  | { type: "command"; command: string }
  | { type: "select_pack"; pack_id: string }
  | { type: "select_proposal"; proposal_id: string }
  | {
      type: "select_verb";
      verb_fqn: string;
      original_input: string;
    }
  | {
      type: "select_entity";
      ref_id: string;
      entity_id: string;
      entity_name: string;
    }
  | {
      type: "select_scope";
      group_id: string;
      group_name: string;
    };

/** Response for session creation */
export interface CreateSessionResponseV2 {
  session_id: string;
  /** Full initial response with ScopeRequired kind for client group selector. */
  response: ReplResponseV2;
}

/** Response for session state query */
export interface SessionStateResponseV2 {
  session_id: string;
  state: ReplStateV2;
  runbook_step_count: number;
  created_at: string;
  last_active_at: string;
}

// ============================================================================
// REPL V2 API Client
// ============================================================================

const V2_BASE = "/repl/v2";

export const replV2Api = {
  /**
   * Create a new V2 REPL session.
   * POST /api/repl/v2/session
   */
  async createSession(): Promise<CreateSessionResponseV2> {
    return api.post<CreateSessionResponseV2>(`${V2_BASE}/session`, {});
  },

  /**
   * Get full session state (for page reload recovery).
   * GET /api/repl/v2/session/:id
   */
  async getSession(sessionId: string): Promise<SessionStateResponseV2> {
    return api.get<SessionStateResponseV2>(`${V2_BASE}/session/${sessionId}`);
  },

  /**
   * Send any input to the V2 REPL (unified endpoint).
   * POST /api/repl/v2/session/:id/input
   */
  async sendInput(
    sessionId: string,
    input: InputRequestV2,
  ): Promise<ReplResponseV2> {
    return api.post<ReplResponseV2>(
      `${V2_BASE}/session/${sessionId}/input`,
      input,
    );
  },

  /**
   * Delete a session.
   * DELETE /api/repl/v2/session/:id
   */
  async deleteSession(sessionId: string): Promise<void> {
    await api.delete(`${V2_BASE}/session/${sessionId}`);
  },

  // =========================================================================
  // Convenience methods
  // =========================================================================

  /** Send a natural language message */
  async sendMessage(
    sessionId: string,
    content: string,
  ): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, { type: "message", content });
  },

  /** Confirm a sentence or runbook */
  async confirm(sessionId: string): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, { type: "confirm" });
  },

  /** Reject a proposed sentence */
  async reject(sessionId: string): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, { type: "reject" });
  },

  /** Select a pack from journey options */
  async selectPack(sessionId: string, packId: string): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, {
      type: "select_pack",
      pack_id: packId,
    });
  },

  /** Select a proposal from the ranked list (Phase 3) */
  async selectProposal(
    sessionId: string,
    proposalId: string,
  ): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, {
      type: "select_proposal",
      proposal_id: proposalId,
    });
  },

  /** Select a verb from disambiguation options */
  async selectVerb(
    sessionId: string,
    verbFqn: string,
    originalInput: string,
  ): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, {
      type: "select_verb",
      verb_fqn: verbFqn,
      original_input: originalInput,
    });
  },

  /** Select an entity to resolve an ambiguous reference */
  async selectEntity(
    sessionId: string,
    refId: string,
    entityId: string,
    entityName: string,
  ): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, {
      type: "select_entity",
      ref_id: refId,
      entity_id: entityId,
      entity_name: entityName,
    });
  },

  /** Select a scope (client group / CBU set) */
  async selectScope(
    sessionId: string,
    groupId: string,
    groupName: string,
  ): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, {
      type: "select_scope",
      group_id: groupId,
      group_name: groupName,
    });
  },

  /** Edit a runbook entry field */
  async editStep(
    sessionId: string,
    stepId: string,
    field: string,
    value: string,
  ): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, {
      type: "edit",
      step_id: stepId,
      field,
      value,
    });
  },

  /** Send a REPL command */
  async sendCommand(
    sessionId: string,
    command: string,
  ): Promise<ReplResponseV2> {
    return this.sendInput(sessionId, { type: "command", command });
  },

  /** Execute the runbook */
  async run(sessionId: string): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, "run");
  },

  /** Undo the last action */
  async undo(sessionId: string): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, "undo");
  },

  /** Clear the runbook */
  async clear(sessionId: string): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, "clear");
  },

  /** Cancel current operation */
  async cancel(sessionId: string): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, "cancel");
  },

  /** Redo the last undone action */
  async redo(sessionId: string): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, "redo");
  },

  /** Show session info */
  async info(sessionId: string): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, "info");
  },

  /** Show help */
  async help(sessionId: string): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, "help");
  },

  /** Disable a runbook step (skip during execution) */
  async disableStep(
    sessionId: string,
    stepId: string,
  ): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, `disable ${stepId}`);
  },

  /** Enable a previously disabled step */
  async enableStep(sessionId: string, stepId: string): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, `enable ${stepId}`);
  },

  /** Toggle disabled state on a step */
  async toggleStep(sessionId: string, stepId: string): Promise<ReplResponseV2> {
    return this.sendCommand(sessionId, `toggle ${stepId}`);
  },
};

export default replV2Api;
