// ============================================================================
// Core Types (matching rust/src/api/intent.rs)
// ============================================================================

/** Parameter value types in intents */
export type ParamValue =
  | string
  | number
  | boolean
  | ParamValue[]
  | { [key: string]: ParamValue };

/** A single verb intent extracted from natural language */
export interface VerbIntent {
  /** The verb to execute, e.g., "cbu.ensure" */
  verb: string;
  /** Parameters with literal values */
  params: Record<string, ParamValue>;
  /** References to previous results, e.g., {"cbu-id": "@last_cbu"} */
  refs: Record<string, string>;
  /** Optional ordering hint */
  sequence?: number;
}

/** Sequence of intents from LLM extraction */
export interface IntentSequence {
  intents: VerbIntent[];
  reasoning?: string;
  confidence?: number;
}

/** Error from intent validation */
export interface IntentError {
  code: string;
  message: string;
  param?: string;
}

/** Result of validating an intent */
export interface IntentValidation {
  valid: boolean;
  intent: VerbIntent;
  errors: IntentError[];
  warnings: string[];
}

/** Assembled DSL from validated intents */
export interface AssembledDsl {
  statements: string[];
  combined: string;
  intent_count: number;
}

// ============================================================================
// Session Types (matching rust/src/api/session.rs)
// ============================================================================

/** Session lifecycle states */
export type SessionState =
  | "new"
  | "pending_validation"
  | "ready_to_execute"
  | "executing"
  | "executed"
  | "closed";

/** Message role */
export type MessageRole = "user" | "agent" | "system";

/** A message in the conversation */
export interface ChatMessage {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: string;
  intents?: VerbIntent[];
  dsl?: string;
}

/** Context maintained across the session */
export interface SessionContext {
  last_cbu_id?: string;
  last_entity_id?: string;
  cbu_ids: string[];
  entity_ids: string[];
  domain_hint?: string;
  named_refs: Record<string, string>;
}

/** Result of executing a single DSL statement */
export interface ExecutionResult {
  statement_index: number;
  dsl: string;
  success: boolean;
  message: string;
  entity_id?: string;
  entity_type?: string;
}

// ============================================================================
// API Request/Response Types
// ============================================================================

export interface CreateSessionRequest {
  domain_hint?: string;
}

export interface CreateSessionResponse {
  session_id: string;
  created_at: string;
  state: SessionState;
}

export interface ChatRequest {
  message: string;
}

export interface ChatResponse {
  message: string;
  intents: VerbIntent[];
  validation_results: IntentValidation[];
  assembled_dsl?: AssembledDsl;
  session_state: SessionState;
  can_execute: boolean;
}

export interface SessionStateResponse {
  session_id: string;
  state: SessionState;
  message_count: number;
  pending_intents: VerbIntent[];
  assembled_dsl: string[];
  combined_dsl: string;
  context: SessionContext;
  messages: ChatMessage[];
  can_execute: boolean;
}

export interface ExecuteRequest {
  dry_run?: boolean;
}

export interface ExecuteResponse {
  success: boolean;
  results: ExecutionResult[];
  errors: string[];
  new_state: SessionState;
}

export interface ClearResponse {
  state: SessionState;
  message: string;
}

// ============================================================================
// UI State
// ============================================================================

export interface AppState {
  sessionId: string | null;
  sessionState: SessionState | null;
  messages: ChatMessage[];
  intents: VerbIntent[];
  validations: IntentValidation[];
  assembledDsl: string[];
  canExecute: boolean;
  loading: boolean;
  error: string | null;

  // Template state
  templates: TemplateSummary[];
  selectedTemplateId: string | null;
  selectedTemplate: FormTemplate | null;
  formValues: Record<string, unknown>;
  renderedDsl: string | null;
  executionLog: string[];
}

// ============================================================================
// Entity Search Types
// ============================================================================

export type EntityType =
  | "CBU"
  | "PERSON"
  | "COMPANY"
  | "TRUST"
  | "DOCUMENT"
  | "PRODUCT"
  | "SERVICE";

export interface EntitySearchRequest {
  query: string;
  types?: EntityType[];
  limit?: number;
  threshold?: number;
  cbu_id?: string;
}

export interface EntityMatch {
  id: string;
  entity_type: EntityType;
  display_name: string;
  subtitle?: string;
  detail?: string;
  score: number;
}

export interface EntitySearchResponse {
  results: EntityMatch[];
  total: number;
  truncated: boolean;
  search_time_ms: number;
}

// ============================================================================
// Template Types
// ============================================================================

export type SlotType =
  | { type: "text"; max_length?: number; multiline?: boolean }
  | { type: "date" }
  | { type: "country" }
  | { type: "currency" }
  | { type: "money"; currency_slot: string }
  | { type: "percentage" }
  | { type: "integer"; min?: number; max?: number }
  | { type: "decimal"; precision?: number }
  | { type: "boolean" }
  | { type: "enum"; options: EnumOption[] }
  | {
      type: "entity_ref";
      allowed_types: EntityType[];
      scope: RefScope;
      allow_create: boolean;
    }
  | { type: "uuid"; auto_generate?: boolean };

export interface EnumOption {
  value: string;
  label: string;
  description?: string;
}

export type RefScope = "global" | "within_cbu" | "within_session";

export interface SlotDefinition {
  name: string;
  label: string;
  slot_type: SlotType;
  required: boolean;
  default_value?: unknown;
  help_text?: string;
  placeholder?: string;
  dsl_param?: string;
}

export interface FormTemplate {
  id: string;
  name: string;
  description: string;
  verb: string;
  domain: string;
  slots: SlotDefinition[];
  tags: string[];
}

export interface TemplateSummary {
  id: string;
  name: string;
  description: string;
  domain: string;
  tags: string[];
}

export interface RenderRequest {
  values: Record<string, unknown>;
}

export interface RenderResponse {
  dsl: string;
  verb: string;
}
