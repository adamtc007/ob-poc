// Shared types for OB-POC panels

export interface SessionResponse {
  session_id: string;
  state: string;
}

export interface ChatResponse {
  stream_id?: string;
  message: string;
  can_execute?: boolean;
  dsl_source?: string;
  ast?: AstStatement[];
  session_state?: string;
  commands?: AgentCommand[];
}

export type AgentCommand =
  | { action: "show_cbu"; cbu_id: string }
  | { action: "highlight_entity"; entity_id: string }
  | { action: "navigate_dsl"; line: number }
  | { action: "focus_ast"; node_id: string };

export interface ExecuteResponse {
  success: boolean;
  results: ExecuteResult[];
  errors: string[];
}

export interface ExecuteResult {
  statement_index: number;
  success: boolean;
  message: string;
  entity_id?: string;
}

export interface CbuSummary {
  cbu_id: string;
  name: string;
  jurisdiction?: string;
  client_type?: string;
}

export interface StreamChunk {
  type: "chunk" | "dsl" | "ast" | "done" | "error";
  content?: string;
  source?: string;
  statements?: AstStatement[];
  can_execute?: boolean;
  message?: string;
}

export interface AstStatement {
  VerbCall?: VerbCallData;
  Comment?: string;
}

export interface VerbCallData {
  domain: string;
  verb: string;
  arguments: AstArgument[];
  binding?: string;
  span: Span;
}

export interface AstArgument {
  key: string;
  value: AstValue;
  span: Span;
}

export type AstValue =
  | { Literal: LiteralValue }
  | { SymbolRef: { name: string; span: Span } }
  | { EntityRef: EntityRefData }
  | AstValue[]
  | { [key: string]: AstValue };

export type LiteralValue =
  | { String: string }
  | { Integer: number }
  | { Decimal: string }
  | { Boolean: boolean }
  | "Null";

export interface EntityRefData {
  entity_type: string;
  search_column?: string;
  value: string;
  resolved_key?: string;
  span: Span;
}

export interface Span {
  start: number;
  end: number;
}

// WASM bridge events
export interface EntitySelectedEvent extends CustomEvent {
  detail: { id: string };
}

export interface FocusEntityEvent extends CustomEvent {
  detail: { id: string };
}

// ============================================================================
// DISAMBIGUATION TYPES
// ============================================================================

/** Disambiguation request - sent when user input is ambiguous */
export interface DisambiguationRequest {
  request_id: string;
  items: DisambiguationItem[];
  prompt: string;
}

/** A single ambiguous item needing resolution */
export type DisambiguationItem =
  | {
      type: "entity_match";
      param: string;
      search_text: string;
      matches: EntityMatch[];
    }
  | {
      type: "interpretation_choice";
      text: string;
      options: Interpretation[];
    };

/** A matching entity for disambiguation */
export interface EntityMatch {
  entity_id: string;
  name: string;
  entity_type: string;
  jurisdiction?: string;
  context?: string;
  score?: number;
}

/** A possible interpretation of ambiguous text */
export interface Interpretation {
  id: string;
  label: string;
  description: string;
  effect?: string;
}

/** User's disambiguation response */
export interface DisambiguationResponse {
  request_id: string;
  selections: DisambiguationSelection[];
}

/** A single disambiguation selection */
export type DisambiguationSelection =
  | { type: "entity"; param: string; entity_id: string }
  | { type: "interpretation"; text: string; interpretation_id: string };

/** Extended chat response that can include disambiguation */
export interface ChatResponseV2 {
  message: string;
  session_state: string;
  payload: ChatPayload;
}

/** Chat response payload - either ready DSL or needs disambiguation */
export type ChatPayload =
  | {
      status: "ready";
      dsl_source: string;
      ast?: AstStatement[];
      can_execute: boolean;
      commands?: AgentCommand[];
    }
  | {
      status: "needs_disambiguation";
      disambiguation: DisambiguationRequest;
    }
  | {
      status: "message";
      commands?: AgentCommand[];
    };
