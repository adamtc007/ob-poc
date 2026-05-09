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
});
