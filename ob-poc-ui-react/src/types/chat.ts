/**
 * Chat and DecisionPacket Types
 *
 * Types for agent chat sessions and the DecisionPacket protocol.
 */

/** Chat message role */
export type MessageRole = 'user' | 'assistant' | 'system';

/** Chat message */
export interface ChatMessage {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: string;
  decision_packet?: DecisionPacket;
  tool_calls?: ToolCall[];
}

/** Tool call information */
export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  result?: unknown;
  status: 'pending' | 'running' | 'success' | 'error';
}

/** Decision packet kind */
export type DecisionKind =
  | 'clarification'
  | 'proposal'
  | 'confirmation'
  | 'result'
  | 'error';

/** Clarification option */
export interface ClarificationOption {
  id: string;
  label: string;
  description?: string;
  value: unknown;
}

/** Clarification payload */
export interface ClarificationPayload {
  question: string;
  context?: string;
  options: ClarificationOption[];
  allow_freeform?: boolean;
}

/** Proposal action */
export interface ProposalAction {
  id: string;
  verb: string;
  description: string;
  args: Record<string, unknown>;
  reversible?: boolean;
}

/** Proposal payload */
export interface ProposalPayload {
  summary: string;
  actions: ProposalAction[];
  requires_confirmation: boolean;
  estimated_impact?: string;
}

/** Confirmation payload */
export interface ConfirmationPayload {
  message: string;
  action_summary: string;
  confirm_button?: string;
  cancel_button?: string;
}

/** Result payload */
export interface ResultPayload {
  success: boolean;
  message: string;
  data?: unknown;
  next_steps?: string[];
}

/** Error payload */
export interface ErrorPayload {
  error: string;
  code?: string;
  recoverable?: boolean;
  suggestions?: string[];
}

/** Decision packet */
export interface DecisionPacket {
  id: string;
  kind: DecisionKind;
  payload:
    | ClarificationPayload
    | ProposalPayload
    | ConfirmationPayload
    | ResultPayload
    | ErrorPayload;
  expires_at?: string;
  confirm_token?: string;
}

/** User reply to a decision packet */
export interface DecisionReply {
  packet_id: string;
  confirm_token?: string;
  selected_option?: string;
  freeform_response?: string;
  confirmed?: boolean;
}

/** Chat session */
export interface ChatSession {
  id: string;
  title?: string;
  created_at: string;
  updated_at: string;
  messages: ChatMessage[];
  context?: SessionContext;
}

/** Session context */
export interface SessionContext {
  cbu_ids?: string[];
  scope?: string;
  dominant_entity_id?: string;
}

/** Chat session summary for listing */
export interface ChatSessionSummary {
  id: string;
  title?: string;
  created_at: string;
  updated_at: string;
  message_count: number;
  last_message_preview?: string;
}

/** Send message request */
export interface SendMessageRequest {
  message: string;
  context?: Record<string, unknown>;
}

/** Send message response */
export interface SendMessageResponse {
  message: ChatMessage;
  session: ChatSession;
}
