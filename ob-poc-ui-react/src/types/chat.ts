/**
 * Chat and DecisionPacket Types
 *
 * Types for agent chat sessions and the DecisionPacket protocol.
 */

/** Chat message role */
export type MessageRole = "user" | "assistant" | "system";

/** Chat message */
export interface ChatMessage {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: string;
  decision_packet?: DecisionPacket;
  tool_calls?: ToolCall[];
  sage_explain?: SageExplain;
  coder_proposal?: CoderProposal;
  discovery_bootstrap?: DiscoveryBootstrap;
  parked_entries?: ParkedEntry[];
  onboarding_state?: OnboardingStateView;
  verb_disambiguation_detail?: VerbDisambiguationRequest;
}

export interface SageExplain {
  understanding: string;
  mode: string;
  scope_summary?: string;
  confidence: string;
  clarifications?: string[];
}

export interface CoderProposal {
  verb_fqn?: string;
  dsl?: string;
  change_summary?: string[];
  requires_confirmation: boolean;
  ready_to_execute: boolean;
}

export interface DiscoveryBootstrap {
  grounding_readiness: string;
  matched_universes?: DiscoveryUniverseOption[];
  matched_domains?: DiscoveryDomainOption[];
  matched_families?: DiscoveryFamilyOption[];
  matched_constellations?: DiscoveryConstellationOption[];
  missing_inputs?: DiscoveryInputPrompt[];
  entry_questions?: DiscoveryQuestionPrompt[];
}

export interface DiscoveryUniverseOption {
  universe_id: string;
  name: string;
  score: number;
}

export interface DiscoveryDomainOption {
  domain_id: string;
  label: string;
  score: number;
}

export interface DiscoveryFamilyOption {
  family_id: string;
  label: string;
  domain_id: string;
  score: number;
}

export interface DiscoveryConstellationOption {
  constellation_id: string;
  label: string;
  score: number;
}

export interface DiscoveryInputPrompt {
  key: string;
  label: string;
  required: boolean;
  input_type: string;
}

export interface DiscoveryQuestionPrompt {
  question_id: string;
  prompt: string;
  maps_to: string;
  priority: number;
}

export interface ParkedEntry {
  step_id: string;
  verb: string;
  park_reason: string;
  correlation_key?: string;
  resource?: string;
  gate_entry_id?: string;
  message?: string;
}

// ============================================================================
// VERB DISAMBIGUATION — "did you mean?" with context
// ============================================================================

export interface VerbDisambiguationRequest {
  request_id: string;
  original_input: string;
  options: VerbDisambiguationOption[];
  prompt: string;
}

export interface VerbDisambiguationOption {
  verb_fqn: string;
  description: string;
  example: string;
  score: number;
  matched_phrase?: string;
  domain_label?: string;
  category_label?: string;
  suggested_utterance?: string;
  // Differentiation — WHY this option differs
  verb_kind?: string; // "primitive" | "macro" | "query" | "workflow"
  differentiation?: string;
  requires_state?: string;
  produces_state?: string;
  scope?: string;
  step_count?: number;
  // Entity/constellation context — WHERE this verb operates
  target_entity_kind?: string;
  constellation_slot?: string;
  entity_context?: string;
  target_entity_name?: string;
}

// ============================================================================
// ONBOARDING STATE VIEW — "where am I + what can I do"
// ============================================================================

export type LayerState = "complete" | "in_progress" | "not_started" | "blocked";
export type VerbDirection = "forward" | "revert" | "query";

export interface OnboardingStateView {
  group_name?: string;
  overall_progress_pct: number;
  active_layer_index: number;
  layers: OnboardingLayer[];
  cbu_cards: CbuStateCard[];
  context_reset_hint?: ContextResetHint;
}

export interface ContextResetHint {
  message: string;
  reset_utterance: string;
  reset_verb_fqn: string;
}

export interface OnboardingLayer {
  index: number;
  name: string;
  description: string;
  state: LayerState;
  progress_pct: number;
  summary?: string;
  forward_verbs: SuggestedVerb[];
  revert_verbs: SuggestedVerb[];
  blocked_verbs: BlockedVerb[];
  unreachable_verbs: UnreachableVerb[];
}

export interface SuggestedVerb {
  verb_fqn: string;
  label: string;
  suggested_utterance: string;
  reason: string;
  boost: number;
  direction: VerbDirection;
  governance_tier?: string;
}

export interface BlockedVerb {
  verb_fqn: string;
  label: string;
  reason: string;
  prerequisite?: string;
  unblock_utterance?: string;
}

export interface UnreachableVerb {
  verb_fqn: string;
  reason: string;
}

export interface CbuStateCard {
  cbu_id: string;
  cbu_name?: string;
  lifecycle_state?: string;
  progress_pct: number;
  phases: CbuPhaseStatus;
  next_action?: SuggestedVerb;
  revert_action?: SuggestedVerb;
}

export interface CbuPhaseStatus {
  has_case: boolean;
  case_status?: string;
  has_screening: boolean;
  screening_complete: boolean;
  document_coverage_pct?: number;
}

export type DiscoverySelectionKind =
  | "domain"
  | "family"
  | "constellation"
  | "question_answer";

export interface DiscoverySelection {
  selection_kind: DiscoverySelectionKind;
  selection_id: string;
  label?: string;
  maps_to?: string;
  value?: string;
}

/** Tool call information */
export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  result?: unknown;
  status: "pending" | "running" | "success" | "error";
}

/** Decision packet kind */
export type DecisionKind =
  | "clarification"
  | "proposal"
  | "confirmation"
  | "result"
  | "error";

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

/** Verb argument profile */
export interface VerbArgProfile {
  name: string;
  arg_type: string;
  required: boolean;
  valid_values?: string[];
  description?: string;
}

/** Verb profile (structured verb universe item) */
export interface VerbProfile {
  fqn: string;
  domain: string;
  description: string;
  sexpr: string;
  args: VerbArgProfile[];
  preconditions_met: boolean;
  governance_tier: string;
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
  available_verbs?: VerbProfile[];
  /** SessionVerbSurface fingerprint — "vs1:<sha256>". Changes when visible verb set changes. */
  surface_fingerprint?: string;
}
