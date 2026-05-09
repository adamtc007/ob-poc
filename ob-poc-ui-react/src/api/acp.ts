import { api } from "./client";

export type AcpAdapterKind = "zed" | "test_harness";
export type AcpPersonaMode = "sage:planning" | "sage:execution";
export type AcpSessionState = "open" | "closed";
export type AcpMutationCapability = "none";
export type ContextClassification =
  | "public"
  | "internal"
  | "confidential"
  | "restricted";

export interface AcpSession {
  session_id: string;
  adapter: AcpAdapterKind;
  persona: AcpPersonaMode;
  state: AcpSessionState;
  opened_at: string;
  mutation_capability: AcpMutationCapability;
}

export interface AcpOpenSessionRequest {
  adapter?: AcpAdapterKind;
  persona?: AcpPersonaMode;
}

export interface AcpOpenSessionResult {
  status: "acp_session_open";
  session: AcpSession;
}

export interface AcpCloseSessionResult {
  status: "acp_session_closed";
  session: AcpSession;
}

export interface AcpCapabilitiesResult {
  status: "acp_capabilities";
  session_id: string;
  capabilities: {
    protocolVersion: string;
    agentCapabilities: {
      loadSession: boolean;
      promptCapabilities: {
        image: boolean;
        audio: boolean;
        embeddedContext: boolean;
      };
      sessionCapabilities: {
        close: boolean;
        list: boolean;
      };
    };
    authMethods: Array<{
      type: string;
      id: string;
      name: string;
      description?: string;
    }>;
    agentInfo: {
      name: string;
      version: string;
    };
    obpocCapabilities?: AcpObpocCapabilities;
  };
  stdio: {
    command: "ob_poc_acp";
    transport: "jsonrpc_stdio";
    message_delimiter: "newline";
  };
}

export interface AcpAdapterPolicy {
  adapter: AcpAdapterKind;
  direct_mutation_supported: boolean;
  mutation_boundary: string;
  policy_authority: string;
}

export interface AcpContextPolicyView {
  max_prompt_classification: ContextClassification;
  allow_external_llm: boolean;
  required_redactions: string[];
}

export interface AcpAuthoritySurfaceDecision {
  surface: string;
  permitted: boolean;
  reason: string;
}

export interface AcpMentionNamespace {
  namespace: string;
  target_kind: string;
  description: string;
}

export interface AcpDeclaredMode {
  mode_id: string;
  label: string;
  description: string;
}

export interface AcpWorkflowPhase {
  phase_id: string;
  label: string;
  description: string;
}

export interface AcpPersonaDeclaration {
  persona_id: AcpPersonaMode;
  label: string;
  description: string;
  mutation_authority: boolean;
}

export interface AcpResourceUriScheme {
  scheme: string;
  resource_kind: string;
  description: string;
}

export interface AcpDeclaredModeCapability extends AcpDeclaredMode {
  discovery_visible: boolean;
  execution_authority: boolean;
}

export interface AcpExternalMcpTransport {
  server_id: string;
  description: string;
  read_only: boolean;
  classification: ContextClassification;
  allowed_probe_ids: string[];
}

export interface AcpTypedExtensionPoint {
  extension_id: string;
  extension_kind: string;
  implementation_ref: string;
}

export interface AcpDiscoveryPolicyDecision {
  probe_id: string;
  operation: string;
  target: string;
  allowed: boolean;
  reason: string;
}

export interface AcpDiscoveryProbe {
  probe_id: string;
  operation: string;
  target: string;
  idempotent: boolean;
  modeled: boolean;
  first_class_state_mutation: boolean;
}

export interface AcpTransitionPolicyDecision {
  transition_ref: string;
  verb: string;
  from_state: string;
  to_state: string;
  dry_run_allowed: boolean;
  mutation_allowed: boolean;
  hitl_required: boolean;
  evidence_refs_required: string[];
  mutation_reason: string;
}

export interface AcpPolicyCapabilities {
  session_id: string;
  pack_id: string;
  pack_version: string;
  compatibility_tier: "dry_run_only" | "reference_mutation" | "reuse_proof";
  adapter_policy: AcpAdapterPolicy;
  authority_surfaces: AcpAuthoritySurfaceDecision[];
  projection_catalog: AcpProjectionCatalogEntry[];
  mention_namespaces: AcpMentionNamespace[];
  declared_modes: AcpDeclaredModeCapability[];
  workflow_phases: AcpWorkflowPhase[];
  acp_personas: AcpPersonaDeclaration[];
  resource_uri_schemes: AcpResourceUriScheme[];
  external_mcp_transports: AcpExternalMcpTransport[];
  typed_extension_points: AcpTypedExtensionPoint[];
  context_policy: AcpContextPolicyView;
  discovery_policy: AcpDiscoveryPolicyDecision[];
  transition_policy: AcpTransitionPolicyDecision[];
}

export interface AcpObpocCapabilities {
  pack: {
    pack_id: string;
    version: string;
    implementation_mode: "native_compiled" | "external_adapter";
    compatibility_tier: "dry_run_only" | "reference_mutation" | "reuse_proof";
  };
  projections: AcpProjectionCatalogEntry[];
  probes: AcpDiscoveryProbe[];
  mentionNamespaces: AcpMentionNamespace[];
  modes: AcpPersonaDeclaration[];
  workflowPhases: AcpWorkflowPhase[];
  resourceUriSchemes: AcpResourceUriScheme[];
  configOptions: {
    personas: AcpPersonaDeclaration[];
    defaultPersona: AcpPersonaMode;
    workflowPhases: AcpWorkflowPhase[];
    classificationTaxonomy: ContextClassification[];
    declinedAuthoritySurfaces: string[];
  };
  classification: AcpContextPolicyView;
  authoritySurfaces: AcpAuthoritySurfaceDecision[];
  externalMcpTransports: AcpExternalMcpTransport[];
  typedExtensionPoints: AcpTypedExtensionPoint[];
}

export interface AcpPolicyResult {
  status: "acp_policy";
  policy: AcpPolicyCapabilities;
}

export type AcpProjectionKind =
  | "pack_manifest"
  | "probe_catalogue"
  | "discovery_surface"
  | "workspace_state"
  | "dag"
  | "graph_scene"
  | "verb_surface"
  | "transition_surface"
  | "governance"
  | "evidence_schema"
  | "affinity_graph"
  | "lineage"
  | "derivation_registry"
  | "materiality"
  | "policy";

export interface AcpProjectionCatalogEntry {
  kind: AcpProjectionKind;
  source: string;
  default_classification: ContextClassification;
  allowed_subject_kinds: string[];
  max_depth?: number | null;
  acp_visible_by_default: boolean;
}

export interface AcpProjectionCatalogResult {
  status: "acp_projection_catalog";
  session_id: string;
  pack_id: string;
  projections: AcpProjectionCatalogEntry[];
}

export interface AcpProjectionSubject {
  subject_kind: string;
  subject_id: string;
}

export interface AcpProjectionRedaction {
  path: string;
  reason: string;
}

export interface AcpProjectionEnvelope {
  projection_kind: AcpProjectionKind;
  session_id: string;
  pack_id: string;
  classification: ContextClassification;
  subject?: AcpProjectionSubject;
  snapshot_refs: string[];
  payload: unknown;
  redactions: AcpProjectionRedaction[];
  projection_hash: string;
  generated_at: string;
}

export interface AcpProjectionResult {
  status: "acp_projection";
  projection: AcpProjectionEnvelope;
}

export interface AcpDiscoveryObservation {
  key: string;
  value: unknown;
  classification?: ContextClassification;
}

export interface AcpDiscoveryProvenance {
  source: string;
  snapshot_ref?: string;
}

export interface AcpContextAssemblyRequest {
  adapter?: AcpAdapterKind;
  probe_id: string;
  subject_kind: string;
  subject_id: string;
  context?: Record<string, unknown>;
  observations?: AcpDiscoveryObservation[];
  provenance?: AcpDiscoveryProvenance[];
  first_class_state_mutated?: boolean;
}

export interface AcpPromptContextObservation {
  key: string;
  value: unknown;
  classification: ContextClassification;
}

export type AcpPromptContextRedactionReason =
  | "classification_limit_exceeded"
  | "required_redaction";

export interface AcpPromptContextRedaction {
  key: string;
  reason: AcpPromptContextRedactionReason;
}

export interface AcpPromptContextAssembly {
  included: AcpPromptContextObservation[];
  redacted: AcpPromptContextRedaction[];
  context_hash: string;
  external_llm_allowed: boolean;
}

export interface AcpSageContextBundle {
  session_id: string;
  pack_id: string;
  probe_id: string;
  prompt_context: AcpPromptContextAssembly;
}

export interface AcpContextAssemblyResult {
  status: "acp_context_assembled";
  bundle: AcpSageContextBundle;
}

export type AcpContentBlock =
  | {
      type: "text";
      text: string;
    }
  | {
      type: "resource_link";
      uri: string;
      name?: string;
      description?: string;
    }
  | {
      type: "embedded_resource";
      uri: string;
      name?: string;
      mime_type?: string;
      text?: string;
    };

export interface AcpGatewayRequest {
  method: string;
  params?: Record<string, unknown>;
}

export interface AcpGatewayResult<T = unknown> {
  status: "acp_gateway_processed";
  session_id: string;
  method: string;
  result: T;
  outgoing: unknown[];
  state_anchor_provider?: Record<string, unknown>;
}

export type AcpPromptDraftSource =
  | "deterministic"
  | "deterministic_draft"
  | "llm"
  | "llm_tool_call"
  | "live_llm";

export interface AcpPromptRequest {
  prompt: AcpContentBlock[];
  draft_source?: AcpPromptDraftSource;
}

export interface AcpPromptResult<T = unknown> {
  status: "acp_prompt_processed";
  session_id: string;
  draft_source?: AcpPromptDraftSource;
  result: T;
  outgoing: unknown[];
  state_anchor_provider?: Record<string, unknown>;
}

export const acpApi = {
  async capabilities(sessionId: string): Promise<AcpCapabilitiesResult> {
    return api.get<AcpCapabilitiesResult>(
      `/session/${sessionId}/acp/capabilities`,
    );
  },

  async policy(sessionId: string): Promise<AcpPolicyResult> {
    return api.get<AcpPolicyResult>(`/session/${sessionId}/acp/policy`);
  },

  async projections(sessionId: string): Promise<AcpProjectionCatalogResult> {
    return api.get<AcpProjectionCatalogResult>(
      `/session/${sessionId}/acp/projections`,
    );
  },

  async projection(
    sessionId: string,
    kind: AcpProjectionKind,
  ): Promise<AcpProjectionResult> {
    return api.get<AcpProjectionResult>(
      `/session/${sessionId}/acp/projections/${kind}`,
    );
  },

  async openSession(
    sessionId: string,
    request: AcpOpenSessionRequest = {},
  ): Promise<AcpOpenSessionResult> {
    return api.post<AcpOpenSessionResult>(
      `/session/${sessionId}/acp/open`,
      request,
    );
  },

  async closeSession(
    sessionId: string,
    request: AcpOpenSessionRequest = {},
  ): Promise<AcpCloseSessionResult> {
    return api.post<AcpCloseSessionResult>(
      `/session/${sessionId}/acp/close`,
      request,
    );
  },

  async assembleContext(
    sessionId: string,
    request: AcpContextAssemblyRequest,
  ): Promise<AcpContextAssemblyResult> {
    return api.post<AcpContextAssemblyResult>(
      `/session/${sessionId}/acp/context`,
      request,
    );
  },

  async gateway<T = unknown>(
    sessionId: string,
    request: AcpGatewayRequest,
  ): Promise<AcpGatewayResult<T>> {
    return api.post<AcpGatewayResult<T>>(
      `/session/${sessionId}/acp/gateway`,
      request,
    );
  },

  async prompt<T = unknown>(
    sessionId: string,
    request: AcpPromptRequest,
  ): Promise<AcpPromptResult<T>> {
    return api.post<AcpPromptResult<T>>(
      `/session/${sessionId}/acp/prompt`,
      request,
    );
  },
};

export default acpApi;
