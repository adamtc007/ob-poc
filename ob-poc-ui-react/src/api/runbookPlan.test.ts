import { afterEach, describe, expect, it, vi } from "vitest";

import {
  runbookPlanApi,
  type ExecutionWorkbook,
  type KycApprovalTokenResult,
  type KycRestrictedMutationCompileRunbookResult,
  type KycRestrictedMutationPreflightResult,
  type KycUpdateStatusDryRunResult,
  type MutationApprovalToken,
  type TraceEntry,
} from "./runbookPlan";

const workbookFixture = (): ExecutionWorkbook => ({
  id: "ewb:v1:workbook",
  core: {
    schema_version: 1,
    pack_id: "ob-poc.kyc",
    transition_ref: "kyc-case.discovery-to-assessment",
    execution_mode: "dry_run",
    session_id: "session-123",
    subject: {
      subject_kind: "kyc_case",
      subject_id: "case-123",
    },
    actor: {
      actor_id: "analyst@example.com",
      roles: ["analyst"],
    },
    configuration_version: "config-1",
    state_snapshot_id: "snapshot-1",
    objective: "Advance the KYC case from DISCOVERY to ASSESSMENT",
    evidence_refs: [
      {
        kind: "case_id",
        ref_id: "case-123",
        digest: "sha256:evidence",
      },
    ],
    simulation: {
      transition_ref: "kyc-case.discovery-to-assessment",
      entity_id: "case-123",
      entity_type: "kyc_case",
      state_machine: "kyc_case_lifecycle",
      from_state: "DISCOVERY",
      to_state: "ASSESSMENT",
      verb: "kyc-case.update-status",
      semantic_diff: {
        field: "status",
        before: "DISCOVERY",
        after: "ASSESSMENT",
      },
      predicted_advance: {
        entity_id: "case-123",
        to_node: "ASSESSMENT",
        slot_path: "kyc-case/workstream",
        reason: "configuration transition",
        writes_since_push_delta: 0,
      },
      state_snapshot_id: "snapshot-1",
      configuration_version: "config-1",
    },
    stale_policy: "revalidate",
    metadata: {},
  },
  status: "draft",
  created_at: "2026-05-05T12:00:00Z",
});

const approvalTokenFixture = (): MutationApprovalToken => ({
  id: "approval:v1:token",
  core: {
    schema_version: 1,
    workbook_id: "ewb:v1:workbook",
    session_id: "session-123",
    pack_id: "ob-poc.kyc",
    transition_ref: "kyc-case.discovery-to-assessment",
    subject: {
      subject_kind: "kyc_case",
      subject_id: "case-123",
    },
    requested_by_actor_id: "analyst@example.com",
    approved_by_actor_id: "approver@example.com",
    approval_text: "Approved for restricted KYC update",
    configuration_version: "config-1",
    state_snapshot_id: "snapshot-1",
    evidence_refs: workbookFixture().core.evidence_refs,
    expires_at: "2099-05-05T13:00:00Z",
  },
  issued_at: "2026-05-05T12:00:00Z",
  status: "active",
});

describe("runbookPlanApi", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("posts a KYC update-status workbook dry-run request", async () => {
    const response: KycUpdateStatusDryRunResult = {
      status: "dry_run_validated",
      workbook: {
        id: "workbook-1",
        core: {
          schema_version: 1,
          pack_id: "ob-poc-kyc",
          transition_ref: "kyc-case.discovery-to-assessment",
          execution_mode: "dry_run",
          session_id: "session-123",
          subject: {
            subject_kind: "case",
            subject_id: "case-123",
          },
          actor: {
            actor_id: "agent-1",
            roles: ["ops"],
          },
          configuration_version: "sem-os-v1",
          state_snapshot_id: "snapshot-1",
          objective: "Advance the KYC case from discovery to assessment",
          evidence_refs: [
            {
              kind: "case-evidence",
              ref_id: "case-123",
              digest: "sha256:evidence",
            },
          ],
          simulation: {
            transition_ref: "kyc-case.discovery-to-assessment",
            entity_id: "case-123",
            entity_type: "kyc_case",
            state_machine: "kyc_case_lifecycle",
            from_state: "discovery",
            to_state: "assessment",
            verb: "kyc-case.update-status",
            semantic_diff: {
              field: "status",
              before: "discovery",
              after: "assessment",
            },
            predicted_advance: {
              entity_id: "case-123",
              to_node: "assessment",
              slot_path: "case.status",
              reason: "configuration transition kyc-case.discovery-to-assessment",
              writes_since_push_delta: 0,
            },
            state_snapshot_id: "snapshot-1",
            configuration_version: "sem-os-v1",
          },
          stale_policy: "reject",
          metadata: {
            source: "test",
          },
        },
        status: "validated",
        created_at: "2026-05-05T12:00:00Z",
      },
      dry_run: {
        workbook_id: "workbook-1",
        transition_ref: "kyc-case.discovery-to-assessment",
        semantic_diff_uri: "semos://semantic-diff/workbook-1",
        validation_trace: [
          {
            step_number: 1,
            step_id: "integrity",
            status: "passed",
            message: "Workbook validated",
          },
        ],
        semantic_diff: {
          transition_ref: "kyc-case.discovery-to-assessment",
          entity_id: "case-123",
          entity_type: "kyc_case",
          state_machine: "kyc_case_lifecycle",
          from_state: "discovery",
          to_state: "assessment",
          verb: "kyc-case.update-status",
          semantic_diff: {
            field: "status",
            before: "discovery",
            after: "assessment",
          },
          predicted_advance: {
            entity_id: "case-123",
            to_node: "assessment",
            slot_path: "case.status",
            reason: "configuration transition kyc-case.discovery-to-assessment",
            writes_since_push_delta: 0,
          },
          state_snapshot_id: "snapshot-1",
          configuration_version: "sem-os-v1",
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

    const result = await runbookPlanApi.dryRunKycUpdateStatusWorkbook(
      "session-123",
      {
        case_id: "case-123",
        current_state: "discovery",
        requested_state: "assessment",
        configuration_version: "sem-os-v1",
        state_snapshot_id: "snapshot-1",
        evidence_digest: "sha256:evidence",
        actor_id: "agent-1",
        actor_roles: ["ops"],
      },
    );

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/session/session-123/workbook/kyc/update-status/dry-run",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          case_id: "case-123",
          current_state: "discovery",
          requested_state: "assessment",
          configuration_version: "sem-os-v1",
          state_snapshot_id: "snapshot-1",
          evidence_digest: "sha256:evidence",
          actor_id: "agent-1",
          actor_roles: ["ops"],
        }),
      },
    );
  });

  it("surfaces KYC dry-run refusal details from the backend", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            error: "simulation_refused",
            detail: "illegal transition discovery -> approved",
          }),
          {
            status: 409,
            statusText: "Conflict",
          },
        ),
      ),
    );

    await expect(
      runbookPlanApi.dryRunKycUpdateStatusWorkbook("session-123", {
        case_id: "case-123",
        current_state: "discovery",
        requested_state: "approved",
        configuration_version: "sem-os-v1",
        state_snapshot_id: "snapshot-1",
        evidence_digest: "sha256:evidence",
        actor_id: "agent-1",
      }),
    ).rejects.toMatchObject({
      name: "ApiError",
      status: 409,
      statusText: "Conflict",
      body: {
        error: "simulation_refused",
        detail: "illegal transition discovery -> approved",
      },
    });
  });

  it("posts a KYC approval token request", async () => {
    const response: KycApprovalTokenResult = {
      status: "approval_token_issued",
      approval_token: approvalTokenFixture(),
    };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const request = {
      workbook: workbookFixture(),
      approved_by_actor_id: "approver@example.com",
      approval_text: "Approved for restricted KYC update",
      expires_at: "2099-05-05T13:00:00Z",
    };
    const result = await runbookPlanApi.issueKycApprovalToken(
      "session-123",
      request,
    );

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/session/session-123/workbook/kyc/approval-token",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(request),
      },
    );
  });

  it("posts a KYC restricted mutation preflight request", async () => {
    const workbook = workbookFixture();
    const approvalToken = approvalTokenFixture();
    const response: KycRestrictedMutationPreflightResult = {
      status: "restricted_mutation_preflight_prepared",
      preflight: {
        workbook_id: workbook.id,
        approval: {
          workbook_id: workbook.id,
          approval_token_id: approvalToken.id,
          transition_ref: workbook.core.transition_ref,
          approved_by_actor_id: "approver@example.com",
          expires_at: "2099-05-05T13:00:00Z",
        },
        verb: "kyc-case.update-status",
        transition_ref: workbook.core.transition_ref,
        intended_diff: {
          subject_id: "case-123",
          field: "status",
          before: "DISCOVERY",
          after: "ASSESSMENT",
        },
        predicted_diff: workbook.core.simulation,
        actual_diff: null,
        executor: "existing_runbook_gate_only",
        runbook_args: {
          "workbook-id": workbook.id,
          "approval-token-id": approvalToken.id,
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
      workbook,
      approval_token: approvalToken,
      observed_configuration_version: workbook.core.configuration_version,
      observed_state_snapshot_id: workbook.core.state_snapshot_id,
      observed_evidence_refs: workbook.core.evidence_refs,
    };
    const result = await runbookPlanApi.prepareKycRestrictedMutationPreflight(
      "session-123",
      request,
    );

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/session/session-123/workbook/kyc/restricted-mutation/preflight",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(request),
      },
    );
  });

  it("posts a KYC restricted mutation compile-runbook request", async () => {
    const workbook = workbookFixture();
    const approvalToken = approvalTokenFixture();
    const preflight = {
      workbook_id: workbook.id,
      approval: {
        workbook_id: workbook.id,
        approval_token_id: approvalToken.id,
        transition_ref: workbook.core.transition_ref,
        approved_by_actor_id: "approver@example.com",
        expires_at: "2099-05-05T13:00:00Z",
      },
      verb: "kyc-case.update-status",
      transition_ref: workbook.core.transition_ref,
      intended_diff: {
        subject_id: "case-123",
        field: "status",
        before: "DISCOVERY",
        after: "ASSESSMENT",
      },
      predicted_diff: workbook.core.simulation,
      actual_diff: null,
      executor: "existing_runbook_gate_only" as const,
      runbook_args: {
        "case-id": "case-123",
        "from-state": "DISCOVERY",
        "to-state": "ASSESSMENT",
        status: "ASSESSMENT",
        "workbook-id": workbook.id,
        "approval-token-id": approvalToken.id,
      },
    };
    const response: KycRestrictedMutationCompileRunbookResult = {
      status: "restricted_mutation_runbook_compiled",
      compilation: {
        compiled_runbook_id: "compiled-runbook-123",
        workbook_id: workbook.id,
        approval_token_id: approvalToken.id,
        transition_ref: workbook.core.transition_ref,
        expected_diff: preflight.intended_diff,
        compiled_runbook: {
          id: "compiled-runbook-123",
          session_id: "session-123",
          version: 3,
          steps: [
            {
              step_id: "step-123",
              sentence: "Apply approved KYC case status update",
              verb: "kyc-case.update-status",
              dsl: '(kyc-case.update-status :case-id "case-123" :status "ASSESSMENT")',
              args: { "case-id": "case-123", status: "ASSESSMENT" },
              depends_on: [],
              execution_mode: "sync",
              write_set: ["case-123"],
              verb_contract_snapshot_id: null,
            },
          ],
          envelope: {},
          status: { status: "compiled" },
          created_at: "2026-05-05T12:00:00Z",
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

    const request = { preflight };
    const result = await runbookPlanApi.compileKycRestrictedMutationRunbook(
      "session-123",
      request,
    );

    expect(result).toEqual(response);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/session/session-123/workbook/kyc/restricted-mutation/compile-runbook",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(request),
      },
    );
  });

  it("returns typed ACP and workbook trace operations", async () => {
    const response: TraceEntry[] = [
      {
        session_id: "session-123",
        sequence: 1,
        timestamp: "2026-05-05T12:00:00Z",
        agent_mode: "sage",
        op: {
          op: "acp_session_opened",
          adapter: "zed",
          mutation_capability: "none",
        },
        stack_snapshot: [],
      },
      {
        session_id: "session-123",
        sequence: 2,
        timestamp: "2026-05-05T12:00:01Z",
        agent_mode: "sage",
        op: {
          op: "acp_context_assembled",
          pack_id: "ob-poc.kyc",
          probe_id: "kyc-case.read-state",
          context_hash: "sha256:context",
          redacted_count: 1,
        },
        stack_snapshot: [],
      },
      {
        session_id: "session-123",
        sequence: 3,
        timestamp: "2026-05-05T12:00:02Z",
        agent_mode: "sage",
        op: {
          op: "acp_projection_served",
          projection_kind: "dag",
          projection_hash: "sha256:projection",
          classification: "internal",
          redacted_count: 0,
          mechanisms: ["projection_get"],
          fallback_summary: [],
          acp_mechanism_summary: ["projection_get", "demand_driven"],
          acp_fallback_summary: [],
          projection_count: 1,
          projection_bytes: 128,
          projection_latency_ms: 3,
        },
        stack_snapshot: [],
      },
      {
        session_id: "session-123",
        sequence: 4,
        timestamp: "2026-05-05T12:00:02Z",
        agent_mode: "dsl_coder",
        op: {
          op: "workbook_dry_run_validated",
          workbook_id: "workbook-1",
          transition_ref: "kyc-case.discovery-to-assessment",
          semantic_diff_uri: "semos://semantic-diff/workbook-1",
          validation_trace: [
            {
              step_number: 3,
              step_id: "integrity",
              status: "passed",
              message: "workbook integrity hash verified",
            },
          ],
        },
        stack_snapshot: [],
      },
      {
        session_id: "session-123",
        sequence: 5,
        timestamp: "2026-05-05T12:00:03Z",
        agent_mode: "repl",
        op: {
          op: "approval_token_issued",
          approval_token_id: "approval:v1:abc",
          workbook_id: "workbook-1",
          approved_by_actor_id: "approver@example.com",
        },
        stack_snapshot: [],
      },
      {
        session_id: "session-123",
        sequence: 6,
        timestamp: "2026-05-05T12:00:04Z",
        agent_mode: "repl",
        op: {
          op: "restricted_mutation_preflight_prepared",
          workbook_id: "workbook-1",
          approval_token_id: "approval:v1:abc",
          transition_ref: "kyc-case.discovery-to-assessment",
        },
        stack_snapshot: [],
      },
      {
        session_id: "session-123",
        sequence: 7,
        timestamp: "2026-05-05T12:00:05Z",
        agent_mode: "sage",
        op: {
          op: "llm_inference_traced",
          trace_id: "trace-1",
          provider: "anthropic",
          model: "claude-sonnet-4-6",
          model_id: "claude-sonnet-4-6",
          prompt_template_version: "sage_outcome_classifier_v2_sonnet_4_6",
          prompt_hash: "sha256:prompt",
          response_hash: "sha256:response",
        },
        stack_snapshot: [],
      },
    ];

    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await runbookPlanApi.getSessionTrace("session-123");

    expect(result).toEqual(response);
    expect(result[1].op.op).toBe("acp_context_assembled");
    if (result[1].op.op === "acp_context_assembled") {
      expect(result[1].op.redacted_count).toBe(1);
      expect(result[1].op.context_hash).toBe("sha256:context");
    }
    expect(result[2].op.op).toBe("acp_projection_served");
    if (result[2].op.op === "acp_projection_served") {
      expect(result[2].op.projection_hash).toBe("sha256:projection");
      expect(result[2].op.acp_mechanism_summary).toContain("demand_driven");
      expect(result[2].op.projection_count).toBe(1);
      expect(result[2].op.projection_bytes).toBe(128);
      expect(result[2].op.projection_latency_ms).toBe(3);
    }
    expect(result[3].op.op).toBe("workbook_dry_run_validated");
    if (result[3].op.op === "workbook_dry_run_validated") {
      expect(result[3].op.semantic_diff_uri).toBe(
        "semos://semantic-diff/workbook-1",
      );
      expect(result[3].op.validation_trace?.[0]?.step_id).toBe("integrity");
    }
    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:3000/api/session/session-123/trace",
      {
        method: "GET",
        headers: { "Content-Type": "application/json" },
      },
    );
  });
});
