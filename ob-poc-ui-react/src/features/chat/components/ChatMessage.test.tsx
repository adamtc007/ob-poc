import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ChatMessage } from "./ChatMessage";
import type { ChatMessage as ChatMessageType } from "../../../types/chat";

describe("ChatMessage ACP trace rendering", () => {
  it("renders structured ACP trace details for HIL review", () => {
    const message: ChatMessageType = {
      id: "msg-1",
      role: "assistant",
      content: "I stopped before executing a dry-run.",
      timestamp: "2026-05-09T12:00:00Z",
      acp_trace: {
        status: "structured_refusal",
        outcome: "structured_refusal",
        outcome_layer: "validation_refusal",
        human_summary: "I stopped because evidence is missing.",
        refusal_code: "missing_evidence_digest",
        needed_from_user: ["evidence_digest"],
        diagnostic_codes: ["missing_evidence_digest"],
        dry_run_valid: false,
        prose_only_failure: false,
        revision_count: 0,
        state_anchor_provider: {
          provider_selected: true,
          provider_id: "deal.update_status.live_deal_state",
          task: "deal.update-status",
          status: "seeded",
          state_anchor_source: "live_read_only_discovery_probe",
          supported_tasks: ["kyc-case.update-status", "deal.update-status"],
          language_pack_generated: true,
          dry_run_valid: false,
          structured_outcome: true,
          no_mutation_authority: true,
        },
      },
    };

    render(<ChatMessage message={message} />);

    expect(screen.getByText("ACP Trace")).toBeInTheDocument();
    expect(screen.getByText("structured_refusal")).toBeInTheDocument();
    expect(screen.getByText("layer: validation_refusal")).toBeInTheDocument();
    expect(screen.getByText("refusal: missing_evidence_digest")).toBeInTheDocument();
    expect(screen.getByText("evidence digest")).toBeInTheDocument();
    expect(screen.getByText("missing_evidence_digest")).toBeInTheDocument();
    expect(screen.getByText("dry-run: not valid")).toBeInTheDocument();
    expect(screen.getByText("prose-only failure: no")).toBeInTheDocument();
    expect(screen.getByText("State Anchor Provider")).toBeInTheDocument();
    expect(screen.getByText("task: deal.update-status")).toBeInTheDocument();
    expect(screen.getByText("provider: seeded")).toBeInTheDocument();
    expect(screen.getByText("anchor: live_read_only_discovery_probe")).toBeInTheDocument();
    expect(screen.getByText("mutation: no authority")).toBeInTheDocument();
  });
});
