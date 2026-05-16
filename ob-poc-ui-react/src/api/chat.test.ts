import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { chatApi } from "./chat";

describe("chatApi session input ACP bridge", () => {
  beforeEach(() => {
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: vi.fn((key: string) => store.get(key) ?? null),
      setItem: vi.fn((key: string, value: string) => {
        store.set(key, value);
      }),
      removeItem: vi.fn((key: string) => {
        store.delete(key);
      }),
      clear: vi.fn(() => {
        store.clear();
      }),
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("surfaces ACP trace from the normal session input path", async () => {
    const inputResponse = {
      kind: "chat",
      response: {
        message: "I validated a dry-run workbook; no mutation ran.",
        session_state: "scoped",
        acp_trace: {
          status: "dry_run_validated",
          outcome: "dry_run_validated",
          route: "session_input",
          provider_task: "deal.update-status",
          requested_draft_source: "deterministic",
          draft_source: "deterministic_provider",
          route_latency_ms: 2,
          route_latency_us: 1500,
          outcome_layer: "dry_run_validated",
          human_summary: "I validated a dry-run workbook; no mutation ran.",
          transition_ref: "deal.prospect-to-qualifying",
          semantic_diff_uri: "semos://semantic-diff/deal-workbook-1",
          dry_run_valid: true,
          first_pass_valid: true,
          revision_count: 0,
          prose_only_failure: false,
          pending_user_turn_required: false,
          estimated_user_repair_turns_avoided: 0,
          performance: {
            total_ms: 2,
            total_us: 1500,
          },
          state_anchor_provider: {
            provider_selected: true,
            provider_id: "deal.update_status.live_deal_state",
            task: "deal.update-status",
            status: "seeded",
            state_anchor_source: "live_read_only_discovery_probe",
            subject_id: "11111111-1111-1111-1111-111111111111",
            supported_tasks: ["kyc-case.update-status", "deal.update-status"],
            needed: [],
            language_pack_generated: true,
            dry_run_valid: true,
            structured_outcome: true,
            no_mutation_authority: true,
          },
        },
      },
    };
    const sessionResponse = {
      id: "session-123",
      created_at: "2026-05-09T10:00:00Z",
      messages: [],
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify(inputResponse), {
          status: 200,
          statusText: "OK",
        }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify(sessionResponse), {
          status: 200,
          statusText: "OK",
        }),
      );
    vi.stubGlobal("fetch", fetchMock);

    const result = await chatApi.sendMessage("session-123", {
      message:
        "Advance deal 11111111-1111-1111-1111-111111111111 from PROSPECT to QUALIFYING with evidence sha256:evidence",
    });

    expect(fetchMock.mock.calls[0][0]).toBe(
      "/api/session/session-123/input",
    );
    expect(fetchMock.mock.calls[0][0]).not.toContain("/acp/kyc/");
    expect(fetchMock.mock.calls[0][0]).not.toContain("/acp/prompt");
    expect(JSON.parse(fetchMock.mock.calls[0][1]?.body as string)).toEqual({
      kind: "utterance",
      message:
        "Advance deal 11111111-1111-1111-1111-111111111111 from PROSPECT to QUALIFYING with evidence sha256:evidence",
    });
    expect(result.message.content).toContain("validated a dry-run workbook");
    expect(result.message.acp_trace).toMatchObject({
      status: "dry_run_validated",
      route: "session_input",
      provider_task: "deal.update-status",
      requested_draft_source: "deterministic",
      draft_source: "deterministic_provider",
      transition_ref: "deal.prospect-to-qualifying",
      semantic_diff_uri: "semos://semantic-diff/deal-workbook-1",
      dry_run_valid: true,
      performance: {
        total_ms: 2,
      },
      state_anchor_provider: {
        provider_id: "deal.update_status.live_deal_state",
        task: "deal.update-status",
        no_mutation_authority: true,
      },
    });
  });
});
