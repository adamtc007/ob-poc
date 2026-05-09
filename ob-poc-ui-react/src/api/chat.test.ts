import { afterEach, describe, expect, it, vi } from "vitest";

import {
  acpPromptTextFromCommand,
  chatApi,
  isAcpPromptCommand,
} from "./chat";

describe("chatApi ACP prompt bridge", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("detects and strips explicit ACP prompt commands", () => {
    expect(isAcpPromptCommand("/acp Move the KYC case")).toBe(true);
    expect(isAcpPromptCommand(" /ACP   Move the KYC case")).toBe(true);
    expect(isAcpPromptCommand("Move the KYC case")).toBe(false);
    expect(acpPromptTextFromCommand("/acp Move the KYC case")).toBe(
      "Move the KYC case",
    );
  });

  it("posts ACP chat utterances through the canonical prompt endpoint", async () => {
    const response = {
      status: "acp_prompt_processed",
      session_id: "session-123",
      result: {
        status: "structured_refusal",
        refusal: {
          refusal_code: "missing_evidence_digest",
        },
        traceProjection: {
          outcome: "structured_refusal",
          outcomeLayer: "validation_refusal",
          humanSummary: "I stopped before dry-run validation.",
          diagnosticCodes: ["missing_evidence_digest"],
          neededFromUser: ["evidence_digest"],
          dryRunValid: false,
          firstPassValid: false,
          revisionCount: 0,
        },
        observability: {
          conversationEfficiency: {
            proseOnlyFailure: false,
            pendingUserTurnRequired: true,
            estimatedUserRepairTurnsAvoided: 1,
          },
        },
      },
      outgoing: [
        {
          method: "session/update",
          params: {
            update: {
              sessionUpdate: "agent_message_chunk",
              content: {
                type: "text",
                text: "I stopped before dry-run validation. I need evidence.",
              },
            },
          },
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

    const result = await chatApi.sendAcpPrompt("session-123", {
      message: "/acp Move KYC case case-123 to DISCOVERY",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/session/session-123/acp/prompt",
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          prompt: [
            {
              type: "text",
              text: "Move KYC case case-123 to DISCOVERY",
            },
          ],
        }),
      },
    );
    expect(fetchMock.mock.calls[0][0]).not.toContain("/acp/kyc/");
    expect(result.message.content).toContain("I stopped before dry-run");
    expect(result.message.acp_trace).toMatchObject({
      status: "structured_refusal",
      outcome: "structured_refusal",
      outcome_layer: "validation_refusal",
      refusal_code: "missing_evidence_digest",
      diagnostic_codes: ["missing_evidence_digest"],
      needed_from_user: ["evidence_digest"],
      dry_run_valid: false,
      prose_only_failure: false,
      pending_user_turn_required: true,
      estimated_user_repair_turns_avoided: 1,
    });
  });

  it("includes read-only KYC case state context when available", async () => {
    const response = {
      status: "acp_prompt_processed",
      session_id: "session-123",
      result: {
        status: "dry_run_validated",
        output: {
          dry_run: {
            transition_ref: "kyc-case.discovery-to-assessment",
            semantic_diff_uri: "semos://semantic-diff/workbook-1",
          },
        },
        traceProjection: {
          outcome: "dry_run_validated",
          outcomeLayer: "dry_run_validated",
          humanSummary: "I validated a dry-run workbook; no mutation ran.",
          transitionRef: "kyc-case.discovery-to-assessment",
          semanticDiffUri: "semos://semantic-diff/workbook-1",
          neededFromUser: [],
          diagnosticCodes: [],
          dryRunValid: true,
          firstPassValid: true,
          revisionCount: 0,
        },
        observability: {
          conversationEfficiency: {
            proseOnlyFailure: false,
            pendingUserTurnRequired: false,
            estimatedUserRepairTurnsAvoided: 0,
          },
        },
      },
      outgoing: [],
    };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(response), {
        status: 200,
        statusText: "OK",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await chatApi.sendAcpPrompt("session-123", {
      message: "/acp Advance the KYC case to ASSESSMENT with evidence sha256:evidence",
      context: {
        acp_state_anchor: {
          subjectId: "11111111-1111-1111-1111-111111111111",
          currentState: "DISCOVERY",
          configurationVersion: "constellation-v1",
          stateSnapshotId: "ui-constellation:case:11111111",
          source: "ui.constellation.read_only",
          snapshotRefs: ["ui-constellation:case:11111111"],
        },
      },
    });

    const body = JSON.parse(fetchMock.mock.calls[0][1]?.body as string);
    expect(body.prompt).toHaveLength(2);
    expect(body.prompt[0]).toEqual({
      type: "text",
      text: "Advance the KYC case to ASSESSMENT with evidence sha256:evidence",
    });
    expect(body.prompt[1]).toMatchObject({
      type: "embedded_resource",
      uri: "semos://entity/11111111-1111-1111-1111-111111111111",
      name: "KYC read-state probe",
      mime_type: "application/json",
    });
    const embedded = JSON.parse(body.prompt[1].text);
    expect(embedded).toMatchObject({
      probe_id: "kyc-case.read-state",
      subject: {
        subject_kind: "kyc_case",
        subject_id: "11111111-1111-1111-1111-111111111111",
      },
      first_class_state_mutated: false,
    });
    expect(embedded.observations).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          key: "case.status",
          value: "DISCOVERY",
          classification: "internal",
        }),
        expect.objectContaining({
          key: "case.configuration_version",
          value: "constellation-v1",
        }),
        expect.objectContaining({
          key: "case.state_snapshot_id",
          value: "ui-constellation:case:11111111",
        }),
      ]),
    );
    expect(fetchMock.mock.calls[0][0]).toBe(
      "/api/session/session-123/acp/prompt",
    );
    expect(fetchMock.mock.calls[0][0]).not.toContain("/acp/kyc/");
    expect(result.message.acp_trace).toMatchObject({
      status: "dry_run_validated",
      outcome: "dry_run_validated",
      transition_ref: "kyc-case.discovery-to-assessment",
      semantic_diff_uri: "semos://semantic-diff/workbook-1",
      dry_run_valid: true,
      prose_only_failure: false,
      pending_user_turn_required: false,
    });
  });
});
