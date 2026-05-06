import { afterEach, describe, expect, it, vi } from "vitest";

import {
  acpApi,
  type AcpContextAssemblyResult,
  type AcpPolicyResult,
} from "./acp";

describe("acpApi", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("fetches ACP capabilities including stdio launch details", async () => {
    const response = {
      status: "acp_capabilities",
      session_id: "session-123",
      capabilities: {
        protocolVersion: "0.4.3",
        agentCapabilities: {
          loadSession: true,
          promptCapabilities: {
            image: false,
            audio: false,
            embeddedContext: true,
          },
          sessionCapabilities: {
            close: true,
            list: true,
          },
        },
        authMethods: [],
        agentInfo: {
          name: "ob-poc-acp",
          version: "0.1.0",
        },
      },
      stdio: {
        command: "ob_poc_acp",
        transport: "jsonrpc_stdio",
        message_delimiter: "newline",
      },
    };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await acpApi.capabilities("session-123");

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:3000/api/session/session-123/acp/capabilities",
      {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      },
    );
  });

  it("fetches ACP-visible SemOS policy decisions", async () => {
    const response: AcpPolicyResult = {
      status: "acp_policy",
      policy: {
        session_id: "session-123",
        pack_id: "ob-poc.kyc",
        pack_version: "1.0.0",
        compatibility_tier: "dry_run_only",
        adapter_policy: {
          adapter: "zed",
          direct_mutation_supported: false,
          mutation_boundary: "workbook_approval_and_compiled_runbook_gate",
          policy_authority: "SemOS Domain Pack + Workbook + Runbook Gate",
        },
        authority_surfaces: [
          {
            surface: "obpoc/projection/get",
            permitted: true,
            reason: "projection payloads are policy-governed",
          },
          {
            surface: "terminal/new",
            permitted: false,
            reason: "terminal execution is outside ACP discovery",
          },
        ],
        projection_catalog: [
          {
            kind: "dag",
            source: "semos.tos.hydrated_state",
            default_classification: "internal",
            allowed_subject_kinds: ["kyc_case"],
            max_depth: 4,
            acp_visible_by_default: true,
          },
        ],
        mention_namespaces: [
          {
            namespace: "entity",
            target_kind: "semos.entity",
            description: "Entity references",
          },
        ],
        declared_modes: [
          {
            mode_id: "discovery",
            label: "Discovery",
            description: "Read-only SemOS discovery",
            discovery_visible: true,
            execution_authority: false,
          },
        ],
        external_mcp_transports: [],
        typed_extension_points: [
          {
            extension_id: "semos.derivation_registry",
            extension_kind: "derivation_registry",
            implementation_ref: "native::sem_os_core::derivation_registry",
          },
        ],
        context_policy: {
          max_prompt_classification: "internal",
          allow_external_llm: false,
          required_redactions: ["case.confidential_evidence.summary"],
        },
        discovery_policy: [
          {
            probe_id: "kyc-case.read-evidence-summary",
            operation: "kyc-case.read",
            target: "kyc_case",
            allowed: true,
            reason: "probe is idempotent, modeled, and read-only",
          },
        ],
        transition_policy: [
          {
            transition_ref: "kyc-case.intake-to-discovery",
            verb: "kyc-case.update-status",
            from_state: "INTAKE",
            to_state: "DISCOVERY",
            dry_run_allowed: true,
            mutation_allowed: false,
            hitl_required: true,
            evidence_refs_required: ["case_id"],
            mutation_reason: "Domain Pack compatibility tier is dry-run only for ACP",
          },
        ],
      },
    };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await acpApi.policy("session-123");

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:3000/api/session/session-123/acp/policy",
      {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      },
    );
  });

  it("fetches ACP projection catalogue", async () => {
    const response = {
      status: "acp_projection_catalog" as const,
      session_id: "session-123",
      pack_id: "ob-poc.kyc",
      projections: [
        {
          kind: "dag" as const,
          source: "semos.tos.hydrated_state",
          default_classification: "internal" as const,
          allowed_subject_kinds: ["kyc_case"],
          max_depth: 4,
          acp_visible_by_default: true,
        },
        {
          kind: "lineage" as const,
          source: "semos.lineage_store",
          default_classification: "confidential" as const,
          allowed_subject_kinds: [],
          max_depth: 3,
          acp_visible_by_default: false,
        },
      ],
    };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await acpApi.projections("session-123");

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:3000/api/session/session-123/acp/projections",
      {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      },
    );
  });

  it("fetches a typed ACP projection envelope", async () => {
    const response = {
      status: "acp_projection" as const,
      projection: {
        projection_kind: "probe_catalogue" as const,
        session_id: "session-123",
        pack_id: "ob-poc.kyc",
        classification: "internal" as const,
        snapshot_refs: ["domain_pack:ob-poc.kyc@0.1.0"],
        payload: {
          probes: [{ probe_id: "kyc-case.read-state" }],
        },
        redactions: [],
        projection_hash: "sha256:abc",
        generated_at: "2026-05-06T00:00:00Z",
      },
    };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await acpApi.projection("session-123", "probe_catalogue");

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:3000/api/session/session-123/acp/projections/probe_catalogue",
      {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      },
    );
  });

  it("opens an ACP session with no mutation capability", async () => {
    const response = {
      status: "acp_session_open",
      session: {
        session_id: "session-123",
        adapter: "zed",
        state: "open",
        opened_at: "2026-05-05T12:00:00Z",
        mutation_capability: "none",
      },
    };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await acpApi.openSession("session-123", {
      adapter: "zed",
    });

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith("/api/session/session-123/acp/open", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ adapter: "zed" }),
    });
  });

  it("closes an ACP session", async () => {
    const response = {
      status: "acp_session_closed",
      session: {
        session_id: "session-123",
        adapter: "zed",
        state: "closed",
        opened_at: "2026-05-05T12:00:00Z",
        mutation_capability: "none",
      },
    };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await acpApi.closeSession("session-123", {
      adapter: "zed",
    });

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/session/session-123/acp/close",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ adapter: "zed" }),
      },
    );
  });

  it("assembles redacted ACP Sage context", async () => {
    const response: AcpContextAssemblyResult = {
      status: "acp_context_assembled",
      bundle: {
        session_id: "session-123",
        pack_id: "ob-poc.kyc",
        probe_id: "kyc-case.read-evidence-summary",
        prompt_context: {
          included: [
            {
              key: "case.status",
              value: "INTAKE",
              classification: "internal",
            },
          ],
          redacted: [
            {
              key: "case.confidential_evidence.summary",
              reason: "required_redaction",
            },
          ],
          context_hash: "sha256:abc",
          external_llm_allowed: false,
        },
      },
    };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const request = {
      adapter: "test_harness" as const,
      probe_id: "kyc-case.read-evidence-summary",
      subject_kind: "kyc_case",
      subject_id: "case-123",
      observations: [
        {
          key: "case.status",
          value: "INTAKE",
          classification: "internal" as const,
        },
      ],
    };

    const result = await acpApi.assembleContext("session-123", request);

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/session/session-123/acp/context",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(request),
      },
    );
  });

  it("surfaces ACP context refusal details", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            error: "DiscoveryRefused { reason: \"unknown probe\" }",
            recoverable: true,
          }),
          {
            status: 409,
            statusText: "Conflict",
          },
        ),
      ),
    );

    await expect(
      acpApi.assembleContext("session-123", {
        probe_id: "kyc-case.write-state",
        subject_kind: "kyc_case",
        subject_id: "case-123",
      }),
    ).rejects.toMatchObject({
      name: "ApiError",
      status: 409,
      statusText: "Conflict",
      body: {
        error: 'DiscoveryRefused { reason: "unknown probe" }',
        recoverable: true,
      },
    });
  });
});
